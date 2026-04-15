## ADDED Requirements

### Requirement: Rust toolchain baseline must be explicit
The repository SHALL define and use an intentional Rust toolchain baseline rather than relying on implicit local or CI defaults.

#### Scenario: Toolchain baseline is reviewed and updated
- **WHEN** the repository performs a toolchain refresh
- **THEN** the project MUST explicitly select the supported Rust baseline
- **AND** local and CI validation MUST target that baseline intentionally

#### Scenario: Toolchain change introduces compile or lint fallout
- **WHEN** a newer Rust baseline changes compiler or lint behavior
- **THEN** the refresh MUST resolve the fallout or defer the baseline increase explicitly
- **AND** the project MUST NOT leave the baseline ambiguous

### Requirement: Dependency upgrades must be deliberate and reviewable
The repository SHALL update dependencies in controlled slices that keep breakage attributable and reversible.

#### Scenario: Direct dependencies are refreshed
- **WHEN** dependencies are upgraded
- **THEN** direct dependencies SHOULD be reviewed and updated intentionally before relying on transitive churn alone
- **AND** the change SHOULD preserve a clear explanation of what was upgraded and why

#### Scenario: Upgrade causes regressions
- **WHEN** a dependency upgrade introduces API, behavior, lint, or test regressions
- **THEN** the refresh MUST either repair the regressions or narrow/revert the problematic upgrade slice

### Requirement: Local and CI dependency behavior must stay aligned enough for reliable validation
The repository SHALL reduce avoidable local-versus-CI dependency and toolchain surprises.

#### Scenario: CI exposes version-skew breakage
- **WHEN** CI resolves dependency or toolchain behavior differently from local development
- **THEN** the refresh MUST treat that as a configuration and validation problem to address
- **AND** the repository SHOULD make the baseline more explicit so the difference does not recur silently

#### Scenario: Refreshed baseline is accepted
- **WHEN** the toolchain and dependency refresh is complete
- **THEN** the resulting baseline MUST be validated in both local and CI-facing workflows
- **AND** the project MUST preserve a green `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings` baseline
