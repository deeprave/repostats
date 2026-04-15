## Why

The repository entered this change in the middle of a large refactor and was not in a trustworthy working state. We needed to restore a compilable baseline, isolate the interrupted refactor for later analysis, and preserve enough context to understand what the refactor was trying to achieve before resuming structural work.

That recovery has now succeeded at the source-tree level, but the change still needs to document the current baseline architecture and the intended direction of the refactor. In particular, the project already contains a built-in plugin model, but the refactor was meant to make external plugin development first-class so plugins could be developed outside this repository and loaded dynamically at runtime.

## What Changes

- Restore the project to a known-good source layout by separating the in-progress refactor from the working tree and re-establishing a stable `src/` baseline.
- Preserve the interrupted refactor in a side location so its intent and useful code can be reviewed and reapplied deliberately.
- Keep and track new workflow and project-context files needed to support OpenSpec-guided recovery work.
- Review non-`src/` refactor changes such as `Cargo.toml` and `build.rs` after the source tree is stabilized, so intentional improvements can be retained without carrying forward a broken code layout.
- Document the current baseline architecture, including the major modules and subsystem relationships in the working `HEAD` source tree.
- Document the difference between the current built-in plugin model and the intended external plugin model.
- Use `../repostats-plugins` as context for the intended external consumer/developer workflow when reviewing the preserved refactor.

## Capabilities

### New Capabilities
- `project-restoration`: Define the repository behaviors required to restore a compilable baseline while preserving interrupted refactor work for later review.
- `architecture-documentation`: Define the documentation work needed to describe the restored baseline architecture, the existing built-in plugin model, and the intended transition to externally developed runtime-loaded plugins.

### Modified Capabilities
- None.

## Impact

- Affected code: `src/`, preserved refactor trees under `../repostats-refactor/`, `Cargo.toml`, `build.rs`, `openspec/`, repository guidance files, and architecture documentation.
- Affected systems: repository layout, build configuration, plugin architecture documentation, and recovery workflow documentation.
- Adjacent context: `../repostats-plugins` as the intended external plugin development project.
- Risk: restoring a working baseline without capturing the architectural intent would lose the reason the refactor existed in the first place.
