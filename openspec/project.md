# Project Context

## Current Project Description

`repostats` is currently a working Rust application and library baseline that is being steered toward a reusable repository-analysis engine with plugin-driven behavior.

The active codebase already contains the major subsystem split that future work is expected to build on:

- `app`
  - CLI startup, argument handling, display, spinner, and application orchestration.
- `core`
  - shared utilities such as logging, validation, retries, versioning, shutdown, and error handling.
- `notifications`
  - event publication and subscriber management for cross-subsystem coordination.
- `plugin`
  - built-in plugin support, plugin lifecycle management, discovery scaffolding, and the external-plugin boundary.
- `queue`
  - internal message queue, publishers, consumers, and typed queue helpers.
- `scanner`
  - repository validation, scan coordination, checkout support, git operations, and scan message generation.

The intended direction remains:

- keep the scanning engine usable as library functionality rather than a single hard-wired CLI flow
- let plugins declare the analysis they need so scanning can be requirement-driven
- preserve plugin-based output handling rather than baking report formats into the scanner itself
- narrow the public library surface so future external consumers depend on stable APIs instead of internal application wiring

## Current Baseline Assessment

As of 2026-04-16, the repository is no longer in the initial "restore a buildable tree" stage. It has progressed to a stable post-recovery baseline with follow-up refactor work landing in the active source tree.

Verified state:

- `cargo test` passes on the active tree
- `cargo clippy --all-targets --all-features -- -D warnings` passes on the active tree
- the workspace contains substantial tests across scanner, queue, notifications, plugin, CLI, and end-to-end paths
- recent completed follow-up work includes:
  - restoring the `HEAD` source baseline and preserving refactor-era trees for review
  - moving `ScannerTask` construction to a builder-based API
  - introducing a thread-safe `gix::ThreadSafeRepository` storage boundary in scanner state
  - cleaning up recursion-heavy scanner helpers
  - introducing stable `NotificationService`, `PluginService`, and `QueueService` facades in subsystem `api.rs` modules
  - migrating primary runtime call sites away from raw global-manager access patterns
  - trimming broad public re-exports and compatibility surface where facade adoption made that safe
  - keeping OpenSpec changes in place to describe remaining intended refactors

Interpretation:

- the repository is functionally healthy enough for deliberate incremental refactor work
- the baseline is now both test-clean and lint-clean, which gives follow-up architectural work a stronger verified starting point
- the next phase is not emergency recovery; it is controlled convergence between the working baseline and the intended engine/plugin architecture

## Recovery Context And Working Assumptions

The preserved refactor material still matters, but it should be treated as design input rather than code to replay wholesale.

Preserved review inputs:

- `../repostats-refactor/src`
  - interrupted refactor source tree preserved during recovery
- `../repostats-refactor/src.new`
  - preserved alternative source snapshot

Working assumptions for future recovery-oriented changes:

- preserve architectural intent where it is already clear in the active tree or OpenSpec artifacts
- prefer small, reviewable changes that improve boundaries without destabilizing the verified baseline
- treat existing service splits, plugin loading scaffolding, build-script changes, and non-trivial dependency additions as potentially intentional unless disproven
- keep the active `src/` tree authoritative; use preserved refactor trees only for comparison and selective extraction

## Progression Since Initial Recovery

The project summary previously described only the first restore milestone. That is now outdated. The current progression is:

1. The original source restore work is complete.
2. The project has a working modular baseline with active `app`, `core`, `notifications`, `plugin`, `queue`, and `scanner` subsystems.
3. Scanner internals have already absorbed targeted refactors from the broader recovery plan.
4. The remaining work is mostly architectural consolidation and surface cleanup rather than restoring basic functionality.

OpenSpec changes that appear materially completed in the active tree:

- `cleanup-restart-project`
- `refactor-scannertask-init`
- `refactor-gix-repository`
- `refactor-recursion-methods`
- `introduce-service-facades`

OpenSpec changes that still describe likely next-stage work:

- `separate-cli-from-library`
- `define-external-plugin-contract`
- `implement-external-plugin-loading`
- `restore-nextest-validation`

## Practical Next Priorities

When choosing follow-up work, prefer items that improve correctness or boundaries without reopening broad recovery risk.

Recommended priorities:

- continue narrowing public API exposure from the now-stable subsystem facade boundaries
- continue separating CLI-facing concerns from the reusable library surface
- define and implement the external plugin contract deliberately, after the internal library boundary is clearer
- restore or adopt the preferred nextest validation workflow on top of the now clean `cargo test` and `clippy` baseline

## Guidance For Ongoing Updates

This file should track the real state of the active repository, not just intended direction.

Update this document when:

- a recovery-stage OpenSpec change lands in the active source tree
- verification status changes materially, especially build, test, nextest, or clippy health
- a previously speculative architectural direction becomes the active baseline
- preserved refactor material is superseded or no longer relevant

Do not let this file drift back into describing only the original restore event. It should remain the canonical short-form answer to:

- what `repostats` is today
- what recovery work has already been completed
- what is still unfinished and why
