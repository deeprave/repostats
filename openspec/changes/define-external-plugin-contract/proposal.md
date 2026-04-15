## Why

The preserved refactor, the exploratory local manifest, and `../repostats-plugins` do not agree on a single external plugin contract. Before resuming loader work, the manifest schema, exported symbol contract, plugin metadata, and version-compatibility rules need to be defined explicitly.

## What Changes

- Define the canonical manifest format for external plugins.
- Define how plugin metadata maps to runtime discovery and command exposure.
- Define the compatibility contract between `repostats` and external plugin crates, including API version expectations.
- Document the expected relationship between `repostats` and `../repostats-plugins`.

## Capabilities

### New Capabilities
- `external-plugin-contract`: Define the manifest, binary, and compatibility contract for externally developed repostats plugins.

### Modified Capabilities
- None.

## Impact

- Affected code: `src/plugin/*`, `plugins/test.yaml`, `../repostats-plugins/*`, and architecture documentation.
- Affected systems: plugin discovery, runtime loading, plugin versioning, and external plugin developer workflow.
