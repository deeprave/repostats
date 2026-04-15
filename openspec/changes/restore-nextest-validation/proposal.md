## Why

The recovered baseline is now buildable and testable with `cargo test`, but local and CI validation are not aligned. `cargo nextest` is temporarily disabled in pre-commit, `clippy` is not enforced locally, and the PR workflow does not yet run the same quality gates we want for routine development.

## What Changes

- Restore `cargo nextest` as a working validation path by fixing or adapting tests that are not robust under `nextest`.
- Re-enable the local `cargo nextest` pre-commit hook once the suite is stable.
- Re-enable the local `cargo clippy` pre-commit hook so local validation matches intended CI quality checks more closely.
- Add explicit `cargo clippy` and `cargo nextest` steps to the PR workflow.
- Review and update project dependencies as needed, then rerun validation and address any regressions caused by newer versions or stricter tooling.

## Capabilities

### New Capabilities
- `validation-alignment`: Define the local and CI validation behavior required to keep pre-commit and PR checks aligned around `clippy` and `nextest`.

### Modified Capabilities
- None.

## Impact

- Affected code: `.pre-commit-config.yaml`, `.github/workflows/pr.yml`, failing tests under `src/`, and dependency declarations in `Cargo.toml`.
- Affected systems: local developer workflow, PR validation, Rust linting, test execution, and dependency maintenance.
