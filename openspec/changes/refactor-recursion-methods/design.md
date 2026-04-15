## Context

The remaining recursion-related clippy findings are limited to private helper methods in `src/scanner/task/git_ops.rs` that accept `&self` but only use it to call themselves recursively. These helpers do not depend on scanner task instance state, so their current method form is misleading and unnecessary.

This is a local refactor with no intended behavior change. The change does not alter scanner inputs, outputs, or traversal semantics; it only makes helper structure reflect the logic more accurately.

## Goals / Non-Goals

**Goals:**
- Remove `&self` from private recursive helpers that do not use instance state.
- Preserve the existing recursive tree traversal and extraction behavior.
- Keep the change localized to scanner git helper implementation details.

**Non-Goals:**
- Changing scanner traversal logic or extraction semantics.
- Refactoring unrelated scanner methods.
- Introducing broader architectural changes.

## Decisions

### Convert recursive helpers to non-instance functions

Private recursive helpers that do not depend on instance state should become private associated functions or private free functions.

Rationale:
- This matches the actual dependency shape of the code.
- It resolves the clippy finding without altering runtime behavior.
- It keeps the refactor small and mechanically verifiable.

Alternative considered:
- Keep the methods as instance methods and suppress the lint.
  - Rejected because the code can be made clearer with a trivial refactor.

## Risks / Trade-offs

- [Small accidental behavior regression in recursion flow] → Keep the refactor mechanical and validate scanner tests after the change.
- [Unclear helper visibility after refactor] → Keep the helpers private to the scanner git operations module or impl scope.

## Migration Plan

1. Convert the affected recursive helpers in `scanner/task/git_ops.rs` to non-instance functions.
2. Update their call sites in the same module.
3. Run scanner tests and clippy validation for the affected area.

Rollback strategy:
- Restore the previous instance-method form if any unexpected regression appears. The change is tightly scoped.

## Open Questions

- None at this time.
