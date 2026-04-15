## Context

The merged `introduce-service-facades` work restored a clean local and CI baseline, but it also exposed a practical problem: dependency behavior is not as predictable across environments as the repository should tolerate.

The PR required a follow-up fix in `src/core/styles.rs` because CI resolved a `colored` version with a different enum shape than the local environment. That is exactly the kind of drift that becomes more common when:

- the Rust toolchain baseline is implicit rather than intentional
- dependencies are updated opportunistically instead of in controlled batches
- local and CI resolution behavior diverge over time

Now that `main` is again green on `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings`, the repository has a good point-in-time baseline for a deliberate refresh. The goal is not “latest at all costs.” The goal is a supported toolchain and dependency set that the project understands, validates, and can keep green.

## Goals / Non-Goals

**Goals:**

- define an explicit Rust toolchain baseline for the project
- refresh dependencies in a controlled, reviewable sequence
- reduce surprises caused by local/CI version skew
- keep test, lint, and CI baselines green throughout the refresh
- document the resulting toolchain and dependency policy so later upgrades are less ad hoc

**Non-Goals:**

- bundling unrelated architecture changes into the upgrade work
- rewriting subsystems merely because newer crates make that possible
- chasing every newest release immediately if it adds churn without value
- replacing core libraries unless the refresh exposes a concrete reason

## Decisions

### Decision: Upgrade in bounded slices, not one large unstructured jump

The refresh should proceed in ordered slices:

1. define and apply the intended Rust toolchain baseline
2. review and update direct dependencies deliberately
3. let transitive dependency updates follow from the direct set
4. repair breakage, lint fallout, and CI drift before widening scope further

Rationale:

- this keeps failures attributable
- it makes rollback practical
- it prevents a single large dependency graph churn from obscuring the real sources of breakage

Alternatives considered:

- run a blanket dependency update and repair everything afterward
  - rejected because it produces poor blameability and high review noise

### Decision: Treat local and CI resolution parity as a design requirement

The refresh should make the project less sensitive to environment-specific resolution differences. That does not require perfectly identical environments, but it does require that the project understand and intentionally manage the versions it relies on.

In practice, this means:

- review whether the repository should pin or constrain some dependencies more explicitly
- ensure CI and local validation both exercise the intended baseline
- avoid code that compiles only because of an incidental local crate version

Rationale:

- the `colored` discrepancy already demonstrated that “works locally” is not sufficient
- predictable resolution behavior improves trust in the validation pipeline

Alternatives considered:

- accept occasional local/CI differences as normal and fix them ad hoc
  - rejected because it turns routine maintenance into recurring incident response

### Decision: Toolchain policy should be explicit after the refresh

The repository should end this change with a clearly documented Rust baseline and a matching CI posture.

That may include:

- a `rust-toolchain.toml` or equivalent explicit version policy
- workflow/tooling updates that use the same baseline intentionally
- documentation in project artifacts if the baseline changed materially

Rationale:

- implicit toolchain drift is hard to reason about
- a stated baseline makes later upgrades intentional instead of accidental

Alternatives considered:

- continue relying on whichever stable toolchain happens to be installed locally or in CI
  - rejected because it weakens reproducibility

### Decision: Validation gates must stay green during the upgrade

This change should preserve:

- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- the existing PR workflow matrix

If `nextest` or local pre-commit alignment also needs adjustment as part of the refresh, that should be coordinated with the existing validation-alignment work rather than lost inside dependency churn.

Rationale:

- a clean baseline is the main asset this change begins with
- upgrades are only useful if the project can verify them consistently

## Risks / Trade-offs

- [Upgrading Rust increases lint or type-system strictness unexpectedly] -> do the toolchain baseline first and fix compiler/lint fallout before broad dependency churn.
- [Dependency updates trigger many small API breakages at once] -> prefer direct-dependency slices and validate after each slice.
- [A newer version is “available” but not worth the churn] -> optimize for supported and stable, not merely newest.
- [Refresh work overlaps with validation-alignment work] -> coordinate changes to CI/pre-commit intentionally rather than letting dependency fallout redefine validation policy implicitly.

## Migration Plan

1. Inventory the current toolchain and direct dependency set.
2. Decide the intended Rust baseline and apply it explicitly.
3. Update direct dependencies in bounded groups, validating after each group.
4. Fix API, lint, and test fallout.
5. Confirm the PR workflow matrix is green with the refreshed baseline.
6. Document the resulting baseline and any policy updates.

Rollback strategy:

- if a dependency or toolchain bump proves too disruptive, revert that bounded slice while retaining any independently safe fixes from earlier slices
