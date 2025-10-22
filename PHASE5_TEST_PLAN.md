# Phase 5: Testing and Validation Plan

## Overview

Validate the parent-directory watching implementation works correctly across all platforms and modes.

## Test Categories

### 1. Unit Tests (Already Passing)
- ✅ All 114 existing tail tests pass
- ✅ No regressions introduced
- ✅ Event resolution logic validated

### 2. Platform-Specific Tests

#### Linux (inotify - PRIMARY TARGET)
**Parent watching should be ACTIVE for --follow=name**

Test scenarios:
- File modification events detected via file watch
- File rename events detected via parent directory watch
- File deletion events detected via parent directory watch
- Multiple files in same directory
- Files in different directories

#### macOS (kqueue - CURRENT PLATFORM)
**Parent watching should be INACTIVE (not beneficial for kqueue)**

Test scenarios:
- File events detected via file watch only
- No parent directory watches added
- Existing behavior maintained

#### FreeBSD/BSD (kqueue)
**Parent watching should be INACTIVE**

Same as macOS - kqueue doesn't benefit from parent watching.

### 3. Mode-Specific Tests

#### --follow=descriptor Mode
**Parent watching should NOT affect this mode**

Expected behavior:
- Events from file watch only
- Rename handling via RenameMode::Both event
- No parent directory events processed

#### --follow=name Mode
**Parent watching ACTIVE on Linux, INACTIVE elsewhere**

Expected behavior:
- Linux: Events from both file and parent watches
- macOS/BSD: Events from file watch only
- Proper event resolution in both cases

### 4. Regression Tests

Critical tests that must pass:
- test_follow_descriptor_vs_rename2
- test_follow_name_retry
- All existing tail tests

## Validation Steps

### Step 1: Local macOS Validation (Current Platform)
```bash
# Should pass - parent watching inactive on macOS
cargo test --test tests test_tail::
```

### Step 2: Linux Container Validation (Target Platform)
```bash
# Use Podman with Ubuntu to test Linux inotify behavior
podman run --rm -v "$PWD:/workspace:z" -w /workspace ubuntu:24.04 bash -c "
  apt-get update -qq && 
  apt-get install -y -qq curl build-essential &&
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y &&
  . \$HOME/.cargo/env &&
  cargo test --test tests test_tail::
"
```

### Step 3: Specific Scenario Tests

Create focused tests for:
1. Parent directory event resolution
2. Watch source tracking
3. Seekable reader functionality
4. Non-seekable reader (stdin) functionality

## Expected Outcomes

### Success Criteria
- ✅ All 114 existing tests pass on macOS
- ✅ All tests pass on Linux (container)
- ✅ Parent watches recorded on Linux + name mode only
- ✅ Event resolution works correctly
- ✅ No performance degradation (<5% overhead)

### Known Limitations (Acceptable)
- Polling mode still has file growth detection issues (separate from this fix)
- BSD platforms don't benefit from parent watching (kqueue limitation)
- Windows behavior unchanged (not primary target)

## Validation Commands

### Quick Smoke Test
```bash
# Build and run a simple tail test
cargo build --package uu_tail --release
echo "test" > test.txt
timeout 5s ./target/release/tail -f test.txt &
TAIL_PID=$!
sleep 1
echo "line 1" >> test.txt
sleep 1
mv test.txt test2.txt
sleep 1
echo "line 2" >> test2.txt
sleep 1
kill $TAIL_PID 2>/dev/null
rm -f test.txt test2.txt
```

### Full Test Suite
```bash
# Run complete test suite
cargo test --test tests test_tail:: --nocapture

# Check for any new warnings
cargo build --package uu_tail 2>&1 | grep warning | wc -l

# Verify only expected warning remains
cargo build --package uu_tail 2>&1 | grep warning
```

### Performance Check
```bash
# Before and after comparison
hyperfine --warmup 3 \
  'echo test | ./target/release/tail -f -n 1' \
  'echo test | tail -f -n 1'
```

## Risk Assessment

### Low Risk
- Changes isolated to follow mode
- Backward compatible
- Platform-specific via cfg
- All tests passing

### Medium Risk
- Container testing may reveal platform-specific issues
- Performance impact unknown without benchmarks
- Event resolution complexity

### Mitigation
- Extensive testing on Linux before merge
- Performance benchmarks on both platforms
- Clear documentation of platform differences
- Rollback plan if issues found

## Next Actions

1. ✅ Run local tests (macOS)
2. 🔄 Run Linux container tests
3. ⏳ Analyze results
4. ⏳ Document any issues found
5. ⏳ Create PR with findings

---
*Test Plan Created: October 22, 2025*
*Status: Ready for Execution*
