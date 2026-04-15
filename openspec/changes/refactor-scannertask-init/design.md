## Context

`ScannerTask::new(...)` currently takes a flat list of constructor arguments that mixes required runtime dependencies with optional and configuration-driven state. In practice, not all of these inputs carry the same weight:

- `QueuePublisher` is a required runtime dependency because normal scanner execution publishes `ScanMessage` output to the queue.
- `notification_manager` can be defaulted from the global notification service in normal runtime usage and is primarily useful as an override for testing or specialized construction.
- `requirements`, `query_params`, and `checkout_manager` are configuration or capability-dependent concerns that are either optional or have meaningful defaults.

The current constructor shape obscures that distinction and is the direct cause of the `clippy::too_many_arguments` warning. It also makes the scanner initialization story harder to understand at call sites.

## Goals / Non-Goals

**Goals:**
- Make the required scanner runtime dependencies explicit in the initialization API.
- Move optional or configuration-driven state into a builder or `with_*` style flow.
- Preserve the existing scanner runtime behavior and queue publishing contract.
- Improve readability of scanner construction call sites.

**Non-Goals:**
- Changing scanner behavior, message formats, or queue semantics.
- Replacing the queue publisher dependency with a derived or lazy-created fallback.
- Solving the `gix::ThreadSafeRepository` boundary change in this artifact; that is handled by a separate change.
- Reworking unrelated scanner validation or notification architecture beyond initialization defaults.

## Decisions

### Keep `QueuePublisher` as a required initialization dependency

`QueuePublisher` remains part of the required scanner initialization contract.

Rationale:
- Normal scanner execution publishes scan output through `publish_message(...)`.
- The publisher is not optional behavior or a test-only concern.
- Keeping it required makes the scanner’s output contract explicit.

Alternative considered:
- Make `QueuePublisher` optional and lazily derive it from global queue state.
  - Rejected because queue output is core scanner behavior, not a fallback.

### Use a dedicated builder for ScannerTask initialization

The public scanner initialization API should use a dedicated builder rather than returning a partially configured `ScannerTask`.

Required builder inputs:
- `scanner_id`
- `repository_path`
- `repository`
- `queue_publisher`

Optional builder configuration:
- `requirements` (default `ScanRequires::NONE`)
- `query_params`
- `checkout_manager`
- `notification_manager` override

Rationale:
- A builder keeps incomplete configuration out of the final task type until `build()` is called.
- It makes the required runtime boundary explicit while keeping optional configuration readable.
- It matches the existing test-oriented builder pattern already present in this code area.

Alternative considered:
- Keep `ScannerTask::new(...)` as the public entry point and return a partially configured task with `with_*` methods.
  - Rejected because it exposes partially initialized task objects and weakens the construction boundary.

### Default `requirements` to `ScanRequires::NONE`

`requirements` should be optional builder state with a default of `ScanRequires::NONE`.

Rationale:
- `ScanRequires::NONE` is already a meaningful default in test construction paths.
- Not all scanner consumers require additional scan capabilities.
- Requirements are configuration-driven, not core runtime infrastructure.

Alternative considered:
- Keep `requirements` required because scanner orchestration often determines it before task creation.
  - Rejected because the task type itself has a valid default and benefits from treating this as optional configuration.

### Allow notification manager to default from global service

`ScannerTask` initialization should support normal runtime creation without requiring an explicit notification manager argument, while still allowing explicit injection for tests or specialized callers.

Rationale:
- This matches existing manager/service patterns elsewhere in the application.
- It removes one argument from the required initialization surface.
- It preserves testability without making the common runtime case noisy.
- The builder or factory layer is the right place to resolve the default before the final task is constructed.

Alternative considered:
- Keep notification manager required everywhere.
  - Rejected because it does not reflect how the service is actually used in normal runtime construction.

### Resolve notification manager defaults in the builder/factory handoff

If no explicit notification manager override is supplied, `build()` should resolve the global notification manager and inject the resolved dependency into the final `ScannerTask`.

Rationale:
- The final task object should receive fully resolved dependencies rather than reaching into global state itself.
- This keeps global service coupling out of the task internals.
- It preserves explicit override behavior for tests.

Alternative considered:
- Resolve the global notification manager inside `ScannerTask` itself.
  - Rejected because it makes the task type implicitly depend on global service state.

### Preserve test ergonomics

Test helpers and compatibility builders should remain concise and should not force tests to manually supply all optional scanner configuration pieces.

Rationale:
- Scanner tests are numerous and should stay easy to read.
- The builder pattern is already familiar in this code area and can be extended cleanly.

Alternative considered:
- Make tests use the full production construction path everywhere.
  - Rejected because it increases boilerplate without improving the scanner contract.

## Risks / Trade-offs

- [Builder growth becomes unstructured] → Keep only genuinely optional/configuration-driven fields in the builder and leave core runtime dependencies explicit.
- [Inconsistent construction paths between runtime and tests] → Share the same builder contract and reserve only thin convenience wrappers for test ergonomics.
- [Overlapping work with the gix repository refactor] → Keep this change focused on initialization semantics and coordinate constructor updates with the repository-boundary refactor when implemented.

## Migration Plan

1. Reduce the required `ScannerTask` constructor surface to core runtime dependencies.
2. Introduce builder or `with_*` configuration for optional scanner state.
3. Default notification manager acquisition from the global service for normal runtime construction.
4. Update `ScannerManager` and scanner test helpers to use the builder-based construction API.
5. Validate scanner tests and clippy behavior for the constructor path.

Rollback strategy:
- Restore the previous flat constructor signature. This is low-risk because the change is limited to scanner initialization paths.
