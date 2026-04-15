# Project Context

## Current Project Description

`repostats` is being reshaped so the core crate behaves like a reusable analysis engine rather than a single hard-wired application flow.

The intended architecture is:

- The core `repostats` library provides the scanning and execution engine.
- Drivers and behavior are plugin-based rather than being tightly coupled to a fixed report pipeline.
- Plugins describe the analysis they need, so scanning should be driven by the needs of the active plugins.
- The data model produced by a scan is derived from plugin requirements and the analysis those plugins request.
- Output is also plugin-based: the resulting analyzed data can be handed to an output plugin that renders or delivers it in the required form.

Examples of output targets include:

- report generation
- email delivery
- spreadsheet export
- any other output implemented by a plugin

## Refactor Recovery Context

The repository is currently in the middle of a large refactor intended to support the engine-and-plugins model above.

The immediate recovery goal is to return the project to a compilable, working state while preserving enough information to understand what the refactor was trying to achieve.

When making recovery decisions:

- preserve architectural intent where it is already clear
- prefer restoring a working baseline before reapplying larger structural changes
- treat refactor-era additions in `Cargo.toml`, `build.rs`, plugin loading, and service-oriented module splits as potentially intentional until verified otherwise

## Current Recovery State

The active repository source tree has been restored to `HEAD` for `src/`, and the application currently builds and tests successfully in that state.

Refactor-era material preserved outside the active project tree:

- `../repostats-refactor/src`
  - the interrupted refactor source tree that had become the active `src/` during recovery
- `../repostats-refactor/src.new`
  - the preserved `src.old/` snapshot that was tested as an alternative restore candidate
- `../repostats-reactor/tests/app_services.rs`
  - a modified test file that did not match the restored `HEAD` source API and blocked `cargo test`

Interpretation:

- the project is working again from the committed `HEAD` source layout
- the preserved refactor trees should be treated as review inputs, not active source
- future recovery work should selectively compare those preserved trees against the restored baseline rather than replacing the baseline wholesale
