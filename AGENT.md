# Guidelines for AI Assistance in uutils/coreutils

## Project Overview
This repository is a cross-platform Rust implementation of GNU Coreutils, aiming for feature parity with GNU while leveraging Rust's safety and performance. Key directories include:
- `src/`: Core utilities (e.g., `ls`, `sort`, `date`) with shared modules in `uucore`.
- `tests/`: Integration and unit tests for POSIX compliance and edge cases like Unicode handling.
- `ci/`: CI workflows for clippy, rustfmt, and cross-compilation.
Focus on maintaining compatibility with GNU behavior, especially in locales (e.g., LANG=C) and error reporting.[web:19][web:48]

## Tools and Integration
- **GitLab Knowledge Graph (gkg) MCP**: Use gkg for querying code relationships, such as "find functions calling `uucore::fs::canonicalize` across utilities" or "analyze data flow in sorting algorithms". Leverage MCP to provide context for audits, e.g., "Audit this module using gkg call graphs for performance bottlenecks".[web:48][web:53][web:57]
- **GitHub CLI (gh)**: Search related issues/PRs with `gh issue list --search "sort unicode"` or cross-repo comparisons via `gh repo clone gnu-mirror/coreutils` and `gh pr list --repo gnu-mirror/coreutils --state closed`. Use for referencing upstream fixes, e.g., "Check PRs in uutils or GNU for similar bugs before suggesting changes".[web:35][web:60]
- **Git Blame**: Always run `git blame <file>` on modified lines to understand previous approaches, e.g., "Why was this unsafe block added? Blame shows it was for legacy compat—suggest safer alternatives". This ensures robustness by preserving intent without breaking existing logic.[web:59][web:61]

## Dos and Don'ts for Code Changes
### Do
- Follow Rust best practices: Use `Result` for errors, borrow checker for zero-cost abstractions, and clippy for lints.
- Ensure GNU parity: Test against GNU outputs with `cargo test -- --test-threads=1` and compare via diff tools.
- Refactor with insight: Use gkg to identify duplicated patterns across utilities; blame commits to retain historical optimizations.
- Search externally: Query gh for open issues in uutils or related Rust crates before implementing features.

### Don't
- Introduce unsafe code without justification—blame history to confirm necessity.
- Ignore cross-platform testing: Always verify on Linux, Windows, macOS using CI.
- Overhaul without context: Avoid repo-wide changes; use gh to check if similar PRs were rejected.
- Skip MCP context: Don't audit in isolation—always pull gkg insights for module interactions.

## Audit and Review Prompts
When assisting with audits, start prompts like: "Using gkg MCP, blame recent changes in src/sort.rs, and search gh for related PRs. Audit for quality (idiomatic Rust), performance (allocation efficiency), and style (rustfmt compliance), suggesting fixes with GNU compat checks."[web:27][web:31][web:61]

## Testing and Validation
Run `cargo fmt --check`, `cargo clippy`, and `cargo test` after changes. Use gkg for coverage analysis: "Query uncovered paths in error handling". Validate against GNU with scripts in `tests/gnu-compat/".[web:28][web:62]
