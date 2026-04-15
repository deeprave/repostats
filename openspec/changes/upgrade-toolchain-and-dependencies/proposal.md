## Why

The recently merged service-facades work exposed version skew between local development and CI, especially around dependency behavior that differed under the PR matrix. The PR is now green, but it also showed that `repostats` is carrying toolchain and dependency drift that should be corrected deliberately rather than waiting for the next surprise break.

The repository now has a clean `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings` baseline on `main`, which makes this the right time to refresh Rust and dependencies in a controlled way.

## What Changes

- Review the current Rust toolchain version and decide whether to raise the project baseline.
- Update Rust dependencies deliberately, prioritizing direct dependencies first and then validating the transitive fallout.
- Align local and CI dependency resolution behavior as much as practical so version-specific surprises are reduced.
- Fix any compile, lint, formatting, or test regressions introduced by the newer toolchain or dependency set.
- Document the new toolchain/dependency baseline once the upgrade is stable.

## Capabilities

### New Capabilities
- `toolchain-refresh`: Define and maintain a current supported Rust/tooling baseline for `repostats`.
- `dependency-refresh`: Define the controlled upgrade path for crate dependencies while preserving a green baseline.

### Modified Capabilities
- None.

## Impact

- Affected code: `Cargo.toml`, `Cargo.lock`, any source or test files affected by upgraded crate APIs or lint behavior, CI workflow/toolchain configuration, and local developer tooling configuration.
- Affected systems: Rust compiler baseline, dependency resolution, local validation, CI matrix behavior, linting, and test execution.
