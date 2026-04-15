## Why

The current baseline exposes global managers directly through `api.rs`, while the preserved refactor shows a cleaner service-wrapper pattern for notifications, plugins, and queues. This can be adopted independently of the larger refactor to improve API boundaries and internal consistency.

## What Changes

- Introduce small service facade types for global notifications, plugin, and queue access.
- Standardize how shared global managers are initialized and borrowed.
- Reduce direct exposure of internal manager construction details through public API modules.

## Capabilities

### New Capabilities
- `service-facades`: Provide consistent service-wrapper access to global repostats subsystems.

### Modified Capabilities
- None.

## Impact

- Affected code: `src/notifications/api.rs`, `src/plugin/api.rs`, `src/queue/api.rs`, and any call sites that depend on global manager access.
- Affected systems: API ergonomics, subsystem boundaries, and service initialization behavior.
