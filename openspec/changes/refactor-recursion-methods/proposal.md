## Why

The remaining scanner-side `clippy::only_used_in_recursion` findings come from private recursive helper methods whose `self` parameter is not actually used for instance state. This is a small code-shape issue, and addressing it now removes the last straightforward scanner-specific clippy cleanup item without changing scanner behavior.

## What Changes

- Refactor recursive scanner helper methods that only use `self` for recursive calls into private associated functions or private free functions.
- Preserve the current behavior of tree counting and recursive extraction logic.
- Keep the change local to scanner git operation helpers with no intended runtime behavior changes.

## Capabilities

### New Capabilities
- `recursion-helper-cleanup`: Define scanner recursive helper structure so recursive tree-processing logic does not require unused instance parameters.

### Modified Capabilities

## Impact

- Affected code:
  - `src/scanner/task/git_ops.rs`
- APIs:
  - private scanner helper method shapes only
- Dependencies:
  - no new dependencies
- Validation:
  - intended to resolve the remaining scanner-side `clippy::only_used_in_recursion` findings
