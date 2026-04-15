## Why

The active baseline still has a placeholder external plugin loader, while the preserved refactor contains a real implementation direction using manifests and dynamic library loading. Once the external plugin contract is defined, that loader work should be resumed as a focused implementation change.

## What Changes

- Implement external plugin discovery from configured search paths.
- Parse plugin manifests and resolve matching shared library files.
- Load external plugins dynamically and validate compatibility before activation.
- Integrate discovered external plugins with the existing plugin manager and built-in discovery flow.

## Capabilities

### New Capabilities
- `external-plugin-loading`: Discover, validate, and dynamically load external repostats plugins at runtime.

### Modified Capabilities
- None.

## Impact

- Affected code: `src/plugin/discovery.rs`, `src/plugin/external/*`, plugin manager integration, and supporting dependency usage in `Cargo.toml`.
- Affected systems: runtime plugin discovery, dynamic loading, manifest parsing, and plugin activation order.
