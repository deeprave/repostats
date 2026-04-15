## 1. Shared Repository Boundary

- [x] 1.1 Update `ScannerTask` to store `gix::ThreadSafeRepository` instead of `gix::Repository`.
- [x] 1.2 Update scanner constructors and test builders so shared scanner state converts repositories to `ThreadSafeRepository` at storage time.
- [x] 1.3 Update `ScannerManager::validate_repository()` and related setup paths to validate with a thread-local repository and convert only before storing shared state.
- [x] 1.4 Update repository-ID and validation helpers in `scanner/manager.rs` to work cleanly across the thread-local/shared repository boundary.

## 2. Scanner Execution Boundary

- [x] 2.1 Replace the current `ScannerTask::repository()` access pattern with a boundary helper or accessor that materializes a thread-local `gix::Repository`.
- [x] 2.2 Update `scanner/task/git_ops.rs` operation entry points to obtain a thread-local repository at method boundaries.
- [x] 2.3 Preserve existing lower-level git helper signatures on `&gix::Repository` wherever possible by passing the materialized thread-local repository through them.
- [x] 2.4 Review scanner code outside `git_ops.rs` for any remaining direct assumptions about a stored `&gix::Repository` and move them behind the same boundary.

## 3. Validation

- [x] 3.1 Update scanner tests and helper construction paths to remain ergonomic with the new shared repository storage type.
- [x] 3.2 Run the scanner-focused test suite and `cargo nextest run --workspace` to confirm repository behavior is unchanged.
- [x] 3.3 Run clippy validation and verify the scanner-side `arc_with_non_send_sync` findings are resolved or reduced as expected.
