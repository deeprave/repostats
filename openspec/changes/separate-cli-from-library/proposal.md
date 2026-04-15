## Why

The project is intended to evolve toward a reusable engine/library with CLI behavior as a separate layer. The preserved refactor shows an attempt to pull CLI code out of `app/cli` and narrow the library-facing surface, but that work needs to be resumed deliberately rather than replayed wholesale.

## What Changes

- Move CLI-specific modules to a clearer top-level CLI layer.
- Reduce coupling between the CLI entry path and the reusable engine/library modules.
- Revisit the public `lib.rs` surface so it reflects the intended library contract more clearly.

## Capabilities

### New Capabilities
- `cli-library-separation`: Separate CLI-facing code from the reusable repostats library surface.

### Modified Capabilities
- None.

## Impact

- Affected code: `src/app/cli/*`, `src/app/*`, `src/main.rs`, `src/lib.rs`, and related core startup modules.
- Affected systems: application structure, library boundaries, and external consumer expectations.
