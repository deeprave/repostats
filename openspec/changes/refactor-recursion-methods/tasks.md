## 1. Recursive Helper Refactor

- [ ] 1.1 Convert `count_tree_entries` in `src/scanner/task/git_ops.rs` from an instance method to a private non-instance recursive helper.
- [ ] 1.2 Convert `extract_tree_recursive` in `src/scanner/task/git_ops.rs` from an instance method to a private non-instance recursive helper.
- [ ] 1.3 Update local call sites in `git_ops.rs` to use the revised helper shape.

## 2. Validation

- [ ] 2.1 Run the relevant scanner tests to confirm recursive tree counting and extraction behavior are unchanged.
- [ ] 2.2 Run clippy validation and verify the `only_used_in_recursion` findings are resolved.
