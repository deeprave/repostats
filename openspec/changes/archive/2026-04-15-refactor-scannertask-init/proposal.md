## Why

`ScannerTask::new(...)` currently carries too many flat constructor arguments and mixes required scanner runtime dependencies with optional or configuration-driven state. This makes scanner initialization harder to read, triggers the current `clippy::too_many_arguments` finding, and obscures which inputs are fundamental to normal scanner operation.

## What Changes

- Refactor `ScannerTask` initialization so required dependencies remain explicit and optional/configured state moves into a clearer builder-style initialization flow.
- Keep `QueuePublisher` as a required constructor dependency because normal scanner operation publishes scan output to the queue.
- Make scanner configuration concerns such as requirements, query parameters, and checkout manager opt-in through builder or `with_*` style configuration rather than flat constructor arguments.
- Allow notification manager injection to remain optional where the global notification service can provide a default for normal runtime usage.

## Capabilities

### New Capabilities
- `scannertask-init-boundary`: Define a clear initialization contract for `ScannerTask` that distinguishes required scanner runtime dependencies from optional or configuration-driven state.

### Modified Capabilities

## Impact

- Affected code:
  - `src/scanner/task/core.rs`
  - `src/scanner/manager.rs`
  - scanner task tests and helper constructors
- APIs:
  - `ScannerTask::new(...)`
  - scanner task builder or initialization helpers
- Dependencies:
  - no new external dependencies are required
- Validation:
  - intended to resolve the current `clippy::too_many_arguments` issue around scanner task construction while making initialization semantics clearer
