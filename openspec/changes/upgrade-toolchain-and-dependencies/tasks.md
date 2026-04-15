## 1. Establish the current baseline

- [x] 1.1 Inventory the current Rust toolchain policy and direct dependency versions.
- [x] 1.2 Review CI and local validation configuration to identify where toolchain or resolution behavior may currently diverge.
- [x] 1.3 Decide whether the project should raise, pin, or otherwise explicitly declare the Rust baseline.

## 2. Refresh the Rust toolchain baseline

- [x] 2.1 Apply the intended Rust toolchain policy in repository configuration.
- [x] 2.2 Update CI or local tooling configuration as needed so the selected baseline is used intentionally.
- [x] 2.3 Fix compiler or lint fallout introduced by the toolchain change before proceeding to broader dependency work.

## 3. Refresh dependencies in bounded slices

- [x] 3.1 Review direct dependencies in `Cargo.toml` and group them into sensible upgrade slices.
- [x] 3.2 Apply the first bounded dependency upgrade slice and regenerate `Cargo.lock`.
- [x] 3.3 Repair API, behavior, formatting, or lint fallout introduced by the upgraded dependencies.
- [x] 3.4 Repeat for remaining dependency slices until the intended refresh is complete.

## 4. Revalidate the refreshed baseline

- [x] 4.1 Run `cargo test`.
- [x] 4.2 Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] 4.3 Confirm the PR workflow matrix remains compatible with the refreshed baseline.

## 5. Document the outcome

- [x] 5.1 Summarize the final Rust baseline and major dependency changes.
- [x] 5.2 Record any deferred upgrades or known follow-up compatibility work.

## Notes

- Baseline inventory completed:
  - local Rust was already `1.90.0`
  - the repository had no explicit toolchain file
  - PR validation used floating `stable` and `beta`
  - dependency duplicates currently include overlapping HTTP/TLS stacks such as `reqwest 0.11/0.12`, `hyper 0.14/1.x`, and `rustls 0.21/0.23`
- First implementation slice chosen:
  - explicitly pin the repository Rust baseline to `1.90.0`
  - change the CI stable lane from floating `stable` to explicit `1.90.0`
  - keep `beta` in the matrix as the forward-compatibility canary
- Toolchain fallout repaired:
  - the repository initially inherited the local `rustup` `complete` profile, which tried to install unavailable optional components for pinned `1.90.0`
  - `rust-toolchain.toml` now sets `profile = "minimal"` and explicitly requests only `rustfmt` and `clippy`
  - after that change, `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings` both passed under the pinned toolchain
- First dependency slice completed:
  - direct dependencies were reviewed and the highest-value slice identified as HTTP/TLS alignment
  - the repository direct dependency on `reqwest` was upgraded from `0.11` to `0.12`
  - `Cargo.lock` was regenerated and the duplicate `reqwest`/`hyper`/`rustls` stacks collapsed to a single `0.12` family
  - no source changes were required; `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings` remained green after the upgrade
- Second dependency slice completed:
  - core runtime and test utility packages were refreshed in `Cargo.lock`, including `tokio`, `serde`, `serde_json`, `thiserror`, `log`, `once_cell`, `tempfile`, and `serial_test`
  - the slice stayed bounded to packages behind stable APIs already used throughout the repository
  - no source changes were required; `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings` remained green after the refresh
- Workflow compatibility spot-check completed:
  - local `cargo +beta check --bins --tests --verbose` completed successfully, matching the command shape used in the PR workflow
  - beta still inherits the machine-level `rustup` profile when invoked explicitly, but the build itself remained compatible with the refreshed baseline
- Final replacement and documentation pass completed:
  - local changes supersede the open Dependabot PRs for `reqwest`, `gix`, `gix-object`, `gix-protocol`, `gix-url`, and `github/codeql-action`
  - `.github/workflows/codeql.yaml` now uses `github/codeql-action@v4`
  - `Cargo.toml` now directly targets `reqwest 0.12`, `gix 0.74`, `gix-object 0.51`, `gix-protocol 0.52`, and `gix-url 0.33.1`
  - one test helper was updated to the current `gix` API by replacing deprecated `peel_to_commit_in_place()` with `peel_to_commit()`
  - the final validation baseline remained green with `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings`
- Deferred follow-up notes:
  - the lockfile still contains some older transitive `gix` family entries, including `gix-url 0.32.0`, because not every indirect dependency path has converged on the latest family yet
  - the broad `cargo update --dry-run` result remains too large for a single safe refresh pass and should continue as bounded follow-up slices if deeper lockfile freshness is desired
