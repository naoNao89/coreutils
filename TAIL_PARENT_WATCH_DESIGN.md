# Tail Parent-Directory Watching - Design Document

## Problem Statement

Current implementation adds parent-directory watching for Linux inotify in `--follow=name` mode to improve rename/delete detection. However, this breaks `--follow=descriptor` mode tests because:

1. **Dual event sources**: Both file and parent directory watches emit events
2. **Event handling assumes single source**: Current logic processes only `event.paths[0]`
3. **State corruption**: Rename tracking gets confused by duplicate/conflicting events
4. **BufReader limitation**: Cannot seek after rename to detect file growth

## Current Architecture Analysis

### Key Components

1. **Observer** (`follow/watch.rs`)
   - Manages watcher lifecycle
   - Tracks files in `FileHandling` struct
   - Contains mode flags: `follow: FollowMode`, `use_polling: bool`

2. **FileHandling** (`follow/files.rs`)
   - HashMap of `PathBuf → PathData`
   - Uses canonicalized absolute paths as keys
   - Tracks last printed file for header logic

3. **PathData** (`follow/files.rs`)
   - `reader: Option<Box<dyn BufRead>>` - **Cannot seek!**
   - `metadata: Option<Metadata>`
   - `display_name: String`

4. **WatcherRx** (`follow/watch.rs`)
   - Wraps `notify::Watcher` and event receiver
   - `watch_with_parent()` adds both file and parent watches (Linux inotify + name mode)

5. **Event Handling** (`handle_event()`)
   - Processes `notify::Event` objects
   - Uses `event.paths[0]` as the event path
   - Updates file state based on event type

### Current Parent-Watching Logic

```rust
// In watch_with_parent() - Linux inotify + follow=name only
if path.is_file() && !use_polling && follow_name {
    // Watch the file itself
    self.watch(&file_path, RecursiveMode::NonRecursive)?;
    
    // ALSO watch parent directory
    if let Some(parent) = path.parent() {
        if parent.is_dir() {
            self.watch(parent, RecursiveMode::NonRecursive)?;
        }
    }
}
```

### Why It Breaks Descriptor Mode

**Scenario: `test_follow_descriptor_vs_rename2`**

```bash
# Setup
echo data > FILE_A
tail -f --follow=descriptor FILE_A &
mv FILE_A FILE_C        # Rename
echo more >> FILE_C     # Append
```

**Expected (main branch - works)**:
1. Rename event: `paths[0] = FILE_A, paths[1] = FILE_C`
2. Handler moves `PathData` from FILE_A → FILE_C in map
3. Modify event: `paths[0] = FILE_C`
4. Handler reads from FILE_C reader, outputs "more"

**Actual (with parent watching - fails)**:
1. Rename events (MULTIPLE):
   - From file watch: `paths[0] = FILE_A, paths[1] = FILE_C`
   - From parent watch: `paths[0] = <parent_dir>` (but contains FILE_A/FILE_C context)
2. Handler processes first event, moves PathData
3. Second event arrives, but state is inconsistent
4. Modify event: handler can't find correct PathData or reader is None

## Proposed Solution

### Phase 1: Watch Source Tracking

Add metadata to track which watch generated each event:

```rust
#[derive(Debug, Clone)]
enum WatchSource {
    File,              // Event from file watch
    ParentDirectory,   // Event from parent directory watch
}

struct WatchedPath {
    file_path: PathBuf,
    parent_path: Option<PathBuf>,  // Only set in Linux inotify + name mode
    source_tracking: HashMap<PathBuf, WatchSource>,  // Maps watch path → source type
}

impl FileHandling {
    // Add watch source info per monitored file
    map: HashMap<PathBuf, (PathData, Option<WatchedPath>)>
}
```

### Phase 2: Event Path Resolution

Map parent directory events to monitored files:

```rust
impl WatcherRx {
    /// Resolve event to the actual monitored file path(s)
    fn resolve_event_paths(&self, event: &notify::Event, files: &FileHandling) 
        -> Vec<(PathBuf, WatchSource)> {
        
        let event_path = event.paths[0];
        let mut resolved = Vec::new();
        
        // Check if event_path is directly monitored
        if files.contains_key(&event_path) {
            resolved.push((event_path.clone(), WatchSource::File));
        }
        
        // Check if event_path is a parent directory of any monitored file
        // (only relevant for Linux inotify + name mode)
        for (file_path, (_, watch_info)) in files.iter() {
            if let Some(watch_info) = watch_info {
                if let Some(parent) = &watch_info.parent_path {
                    if parent == &event_path {
                        // Event from parent directory - check if it affects this file
                        if event_affects_file(event, file_path) {
                            resolved.push((file_path.clone(), WatchSource::ParentDirectory));
                        }
                    }
                }
            }
        }
        
        resolved
    }
}

fn event_affects_file(event: &notify::Event, file_path: &Path) -> bool {
    // Check if any path in event.paths matches file_path or references it
    event.paths.iter().any(|p| p == file_path || 
                                  p.file_name() == file_path.file_name())
}
```

### Phase 3: Mode-Specific Event Handling

Separate logic for descriptor vs name modes:

```rust
impl Observer {
    fn handle_event(&mut self, event: &notify::Event, settings: &Settings) 
        -> UResult<Vec<PathBuf>> {
        
        // Resolve event to affected file(s)
        let resolved_paths = self.watcher_rx.as_ref().unwrap()
            .resolve_event_paths(event, &self.files);
        
        if resolved_paths.is_empty() {
            return Ok(vec![]);  // Event doesn't affect monitored files
        }
        
        let mut result_paths = Vec::new();
        
        for (file_path, watch_source) in resolved_paths {
            match self.follow {
                Some(FollowMode::Descriptor) => {
                    result_paths.extend(
                        self.handle_descriptor_event(event, &file_path, settings)?
                    );
                }
                Some(FollowMode::Name) => {
                    result_paths.extend(
                        self.handle_name_event(event, &file_path, watch_source, settings)?
                    );
                }
                None => {}
            }
        }
        
        Ok(result_paths)
    }
    
    fn handle_descriptor_event(&mut self, event: &notify::Event, file_path: &Path, 
                                settings: &Settings) -> UResult<Vec<PathBuf>> {
        // Current logic - expects events ONLY from file watch
        // No parent directory event handling
        // ...existing implementation...
    }
    
    fn handle_name_event(&mut self, event: &notify::Event, file_path: &Path,
                         watch_source: WatchSource, settings: &Settings) 
        -> UResult<Vec<PathBuf>> {
        
        use notify::event::*;
        
        match (event.kind, watch_source) {
            // File events - handle modifications
            (EventKind::Modify(ModifyKind::Data(_)), WatchSource::File) => {
                // Read new data from file
                // ...
            }
            
            // Parent events - handle renames/deletes
            (EventKind::Modify(ModifyKind::Name(_)), WatchSource::ParentDirectory) |
            (EventKind::Remove(_), WatchSource::ParentDirectory) => {
                // File was renamed or deleted
                // Update watches and metadata
                // ...
            }
            
            // Ignore duplicate events
            (EventKind::Modify(ModifyKind::Data(_)), WatchSource::ParentDirectory) => {
                // Parent dir doesn't generate data events for children
                return Ok(vec![]);
            }
            
            _ => {
                // Handle other event types
                // ...
            }
        }
        
        Ok(vec![file_path.to_owned()])
    }
}
```

### Phase 4: Seekable Reader

Replace `Box<dyn BufRead>` with trait object supporting both BufRead and Seek:

```rust
// Define combined trait
trait BufReadSeek: BufRead + Seek + Send {}
impl<T: BufRead + Seek + Send> BufReadSeek for T {}

pub struct PathData {
    // OLD: reader: Option<Box<dyn BufRead>>,
    // NEW:
    pub reader: Option<Box<dyn BufReadSeek>>,
    pub metadata: Option<Metadata>,
    pub display_name: String,
}

impl FileHandling {
    pub fn update_reader(&mut self, path: &Path) -> UResult<()> {
        // After rename, seek to current position to detect growth
        if let Some(reader) = self.get_mut(path).reader.as_mut() {
            let current_pos = reader.stream_position()?;
            reader.seek(SeekFrom::Start(current_pos))?;
        } else {
            // Reopen file
            self.get_mut(path)
                .reader
                .replace(Box::new(BufReader::new(File::open(path)?)));
        }
        Ok(())
    }
    
    pub fn seek_to_end(&mut self, path: &Path) -> UResult<()> {
        if let Some(reader) = self.get_mut(path).reader.as_mut() {
            reader.seek(SeekFrom::End(0))?;
        }
        Ok(())
    }
}
```

### Phase 5: Stdin Handling

stdin cannot seek, so maintain separate code path:

```rust
impl PathData {
    pub fn new(
        reader: Option<Box<dyn BufRead>>,
        metadata: Option<Metadata>,
        display_name: &str,
        is_stdin: bool,
    ) -> Self {
        let seekable_reader = if is_stdin {
            // stdin is not seekable - wrap in adapter
            reader.map(|r| Box::new(NonSeekableAdapter::new(r)) as Box<dyn BufReadSeek>)
        } else {
            // Regular files are seekable
            reader.map(|r| {
                // BufReader<File> already implements Seek
                Box::new(r) as Box<dyn BufReadSeek>
            })
        };
        
        Self {
            reader: seekable_reader,
            metadata,
            display_name: display_name.to_owned(),
        }
    }
}

// Adapter for non-seekable readers
struct NonSeekableAdapter<R: BufRead> {
    inner: R,
}

impl<R: BufRead> NonSeekableAdapter<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: BufRead> BufRead for NonSeekableAdapter<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.inner.fill_buf()
    }
    
    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }
}

impl<R: BufRead> Read for NonSeekableAdapter<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<R: BufRead> Seek for NonSeekableAdapter<R> {
    fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
        // No-op for stdin
        Ok(0)
    }
}
```

## Implementation Plan

### Step 1: Add Watch Source Tracking (Non-Breaking)
- Add `WatchedPath` and `WatchSource` types
- Update `FileHandling` to store watch metadata
- Update `watch_with_parent()` to record parent path
- **Validate**: Existing tests still pass

### Step 2: Implement Event Path Resolution (Non-Breaking)
- Add `resolve_event_paths()` method
- Keep existing event handling as fallback
- **Validate**: Existing tests still pass

### Step 3: Split Event Handling by Mode
- Create `handle_descriptor_event()` with current logic
- Create `handle_name_event()` with dual-watch awareness
- Update `handle_event()` to dispatch based on mode
- **Validate**: Run descriptor mode tests

### Step 4: Refactor Reader to Support Seek
- Define `BufReadSeek` trait
- Update `PathData` with new reader type
- Add `NonSeekableAdapter` for stdin
- Update all reader creation sites
- **Validate**: Run full test suite

### Step 5: Enable Parent Watching for Name Mode
- Ensure `watch_with_parent()` is active for Linux + name mode
- Implement parent event filtering in `handle_name_event()`
- **Validate**: Run name mode tests in Linux container

### Step 6: Cross-Platform Testing
- Test on Linux (inotify)
- Test on macOS (kqueue)
- Test on FreeBSD (kqueue) - if available
- **Validate**: All platform tests pass

## Testing Strategy

### Unit Tests
- Test `resolve_event_paths()` with various event types
- Test `WatchSource` tracking correctness
- Test `BufReadSeek` trait implementation

### Integration Tests
- `test_follow_descriptor_vs_rename2` (must pass)
- `test_follow_name_retry` (existing)
- New test: `test_follow_name_parent_events`
- New test: `test_follow_descriptor_no_parent_events`

### Platform-Specific Tests
```rust
#[cfg(target_os = "linux")]
#[test]
fn test_linux_inotify_parent_watch() {
    // Verify parent watch is added for name mode
    // Verify events from both sources are handled correctly
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
#[test]
fn test_kqueue_file_watch_only() {
    // Verify only file is watched (no parent)
    // Verify tests pass without parent watching
}
```

### Container Testing
```bash
# Linux (inotify)
podman run --rm -v "$PWD:/workspace:z" -w /workspace ubuntu:24.04 bash -c "
  # ... setup rust ...
  cargo test --features feat_os_unix test_tail
"

# FreeBSD (kqueue) - if available
# Similar container test
```

## Risks and Mitigations

### Risk: Seekable Reader Breaks stdin
**Mitigation**: NonSeekableAdapter provides no-op Seek for stdin

### Risk: Event Resolution Too Slow
**Mitigation**: Cache file→parent mappings, use HashMap lookups

### Risk: Platform Differences
**Mitigation**: Extensive conditional compilation, platform-specific tests

### Risk: Regression in Descriptor Mode
**Mitigation**: Keep descriptor mode logic isolated, validate with existing tests

## Success Criteria

1. ✅ All existing tests pass (no regressions)
2. ✅ `test_follow_descriptor_vs_rename2` passes with parent watching code present
3. ✅ New name-mode tests pass on Linux
4. ✅ Tests pass on macOS and BSD without modifications
5. ✅ Performance impact < 5% (benchmark existing vs new)
6. ✅ Code coverage > 80% for new paths

## Timeline Estimate

- Phase 1: Watch Source Tracking - 2 days
- Phase 2: Event Path Resolution - 2 days  
- Phase 3: Mode-Specific Handling - 3 days
- Phase 4: Seekable Reader - 3 days
- Phase 5: Testing & Validation - 4 days
- **Total: ~2 weeks** (assuming focused development)

## Open Questions

1. Should we support parent watching on BSD/macOS even though kqueue has limitations?
   - **Recommendation**: No, only Linux inotify benefits from this
   
2. What if parent directory is not accessible (permissions)?
   - **Recommendation**: Fall back to file-only watching, show warning

3. Should polling mode ever use parent watching?
   - **Recommendation**: No, polling doesn't benefit from inotify-specific workarounds

---
*Design Version: 1.0*
*Last Updated: October 22, 2025*
