## Context

The scanner subsystem follows the same shared-service pattern as the rest of the application: a configured manager is created once, wrapped in `Arc`, and used throughout the system. `ScannerManager` currently stores `Arc<ScannerTask>` instances, and `ScannerTask` stores a `gix::Repository` directly. That repository handle is not `Sync`, which means the shared scanner graph does not satisfy the thread-safety expectations implied by `Arc` and triggers the remaining scanner-side `clippy::arc_with_non_send_sync` failures.

The `gix` crate already defines the intended split for this situation:
- `gix::Repository` is a thread-local repository handle
- `gix::ThreadSafeRepository` is the shareable boundary type for `Send`/`Sync` contexts

The scanner codebase is already centralized enough to make this practical. Repository creation and validation are concentrated in `src/scanner/manager.rs`, while most actual git operations live in `src/scanner/task/git_ops.rs`. Most helpers already accept `&gix::Repository`, which allows this change to focus on where repository handles are stored and materialized rather than rewriting the git logic itself.

## Goals / Non-Goals

**Goals:**
- Move shared scanner state from `gix::Repository` to `gix::ThreadSafeRepository`.
- Preserve the existing shared-service pattern for `ScannerManager` and `ScannerTask`.
- Keep lower-level git helper APIs mostly unchanged by materializing thread-local repositories at execution boundaries.
- Reduce the scanner-side `arc_with_non_send_sync` validation issues without changing the git library stack.

**Non-Goals:**
- Replacing `gix` with another git library.
- Reworking scanner behavior, filtering, traversal, or diff-analysis semantics.
- Solving all remaining clippy issues in the scanner subsystem.
- Introducing a new global service pattern or changing the ownership model away from `Arc`.

## Decisions

### Store `gix::ThreadSafeRepository` in shared scanner state

`ScannerTask` will store `gix::ThreadSafeRepository` instead of `gix::Repository`.

Rationale:
- This matches the `gix` crate’s intended usage for shared contexts.
- It preserves the existing service pattern instead of forcing a redesign around single-thread ownership.
- It localizes the refactor to the repository boundary rather than the scanner lifecycle.

Alternative considered:
- Keep `gix::Repository` in shared state and suppress `arc_with_non_send_sync`.
  - Rejected because the crate explicitly provides a safer boundary type for this scenario.

### Convert to thread-safe form at storage time, not at use sites

Repository discovery and immediate validation will continue to use a normal `gix::Repository` in `ScannerManager`. Once validation is complete and the repository is about to be stored in shared scanner state, it will be converted with `into_sync()`.

Rationale:
- Validation APIs already work against `&gix::Repository`.
- It avoids pushing `ThreadSafeRepository` into code paths that do not need shared storage.
- It keeps repository opening/discovery logic easy to reason about.

Alternative considered:
- Convert to `ThreadSafeRepository` immediately on discovery, then convert back everywhere.
  - Rejected because it pushes the shared type too early into local validation paths.

### Materialize thread-local repositories at git operation boundaries

Git operations in `git_ops.rs` will obtain a thread-local repository from the stored `ThreadSafeRepository` at method entry, then continue passing `&gix::Repository` through existing helper methods.

Rationale:
- Most helper signatures can remain unchanged.
- The scanning implementation stays readable and avoids propagating `ThreadSafeRepository` everywhere.
- It aligns with `gix`’s own model: shared repository at the boundary, thread-local repository during actual work.

Alternative considered:
- Change all helper functions to operate directly on `ThreadSafeRepository`.
  - Rejected because it would widen the refactor surface unnecessarily.

### Change the repository accessor shape

`ScannerTask::repository()` cannot continue returning `&gix::Repository`. It should be replaced by a method that creates a thread-local repository when needed, or by a closure-based helper that scopes the thread-local handle to one operation.

Preferred direction:
- a small helper that materializes a local repository for a specific operation boundary

Rationale:
- Prevents accidental long-lived borrowing assumptions from shared state.
- Makes the storage/execution split explicit in the API.

Alternative considered:
- Expose the `ThreadSafeRepository` directly and let every caller convert it manually.
  - Rejected because it would spread boundary management across too many call sites.

## Risks / Trade-offs

- [Boundary churn in scanner code] → Keep low-level helper functions on `&gix::Repository` and change only acquisition points.
- [Unexpected `gix` API friction around conversion] → Start with manager/task construction and one git operation path before broadening the mechanical changes.
- [Test breakage from constructor changes] → Preserve test ergonomics by keeping constructors/builder paths accepting `gix::Repository` and converting internally where practical.
- [Residual non-`Sync` issues beyond the repository field] → Re-run validation after the repository refactor and treat any remaining issues as separate architectural findings.

## Migration Plan

1. Update `ScannerTask` to store `gix::ThreadSafeRepository`.
2. Update `ScannerManager` repository discovery/validation paths to convert repositories to thread-safe form before storing them in shared state.
3. Replace the current repository accessor with a boundary helper that materializes a thread-local repository for git operation entry points.
4. Update `git_ops.rs` method entry points to use the new helper while preserving existing lower-level helper signatures.
5. Run scanner tests, `cargo nextest`, and the clippy validation path to verify the scanner-side `arc_with_non_send_sync` issues are resolved or reduced as expected.

Rollback strategy:
- Revert the storage type back to `gix::Repository` and restore the old accessor shape. This is low-risk because the change is concentrated in scanner internals.

## Outcome Notes

Implemented outcome:
- `ScannerTask` now stores `gix::ThreadSafeRepository` and exposes a simpler thread-local repository accessor.
- `ScannerManager::validate_repository()` continues returning a thread-local `gix::Repository`, with conversion happening at shared-state storage time.
- Scanner git operation entry points and cleanup paths were moved behind the same repository materialization boundary.
