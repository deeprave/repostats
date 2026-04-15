## Why

The scanner service currently stores `gix::Repository` inside shared resources such as `ScannerTask`, which makes the shared scanner service graph fail `Send`/`Sync` expectations and blocks stricter validation. `gix` already provides `ThreadSafeRepository` for this exact boundary, so this change aligns the scanner architecture with the intended shared-service pattern without replacing the existing git library stack.

## What Changes

- Replace shared scanner-side storage of `gix::Repository` with `gix::ThreadSafeRepository` at the service boundary.
- Update scanner construction and validation paths to perform repository discovery and validation with a thread-local `Repository`, then convert to `ThreadSafeRepository` before storing shared state.
- Refactor scanner task execution boundaries so git operations materialize a thread-local repository when needed and continue to pass `&gix::Repository` through existing helper methods.
- Preserve the current shared-service model for `ScannerManager` and `ScannerTask` while removing the non-`Sync` repository handle from long-lived shared state.

## Capabilities

### New Capabilities
- `thread-safe-gix-boundary`: Define a thread-safe scanner repository boundary using `gix::ThreadSafeRepository` for shared services and thread-local `gix::Repository` handles for git operations.

### Modified Capabilities

## Impact

- Affected code:
  - `src/scanner/manager.rs`
  - `src/scanner/task/core.rs`
  - `src/scanner/task/git_ops.rs`
  - scanner tests and helper constructors
- APIs:
  - internal scanner construction and repository access patterns
  - `ScannerTask::repository()` and related helpers will change shape
- Dependencies:
  - continues using the existing `gix` crate family; no git library replacement is planned
- Validation:
  - supports resolving the current scanner-side `arc_with_non_send_sync` clippy issues by using the thread-safe repository type intended by `gix`
