## Context

The repository contained an interrupted refactor that appears to be moving `repostats` toward a library-first engine with plugin-driven analysis and output. The recovery work has now restored `src/` from `HEAD`, producing a buildable and testable baseline, while preserving refactor-era material outside the active tree.

The preserved review inputs are:

- `../repostats-refactor/src`: interrupted refactor source tree
- `../repostats-refactor/src.new`: preserved `src.old/` snapshot that was tested as an alternative restore candidate
- `../repostats-refactor/tests/app_services.rs`: test drift that blocked the restored baseline test suite

The immediate need is no longer source-tree rescue. It is to consolidate the recovery outcome, review remaining non-`src/` changes, and document the current architecture and intended refactor direction before more structural changes are attempted.

## Goals / Non-Goals

**Goals:**
- Preserve the restored `HEAD` baseline as the active working source tree.
- Preserve the interrupted refactor trees outside the primary source path for later comparison.
- Keep OpenSpec and repository guidance files in the working repository.
- Review `Cargo.toml`, `build.rs`, and other non-`src/` changes after the source restore so intentional improvements can be reintroduced deliberately.
- Document the current baseline architecture and subsystem relationships.
- Document the gap between the current built-in plugin model and the intended external plugin model.
- Leave a clear artifact trail describing what was restored, what was preserved, and what remains to be reviewed.

**Non-Goals:**
- Finishing the plugin-oriented refactor in this change.
- Resolving every architectural question about the long-term engine/plugin model.
- Keeping the current mixed working tree intact as the primary baseline.
- Collapsing refactor recovery and refactor completion into one implementation pass.

## Decisions

### Restore `src/` from `HEAD` as the baseline source tree
The successful recovery baseline is the committed `HEAD` source tree, not `src.old/`. `src.old/` was useful as preserved context for some subsystems, but it was not a full project restore point.

Alternative considered:
- Promote `src.old/` into the active baseline permanently. Rejected because it lacked the full application/core entrypoint structure required for a complete working build.

### Preserve both refactor-era source snapshots outside the active tree
The interrupted refactor still contains useful intent and potentially useful code, and `src.old/` also contains preserved subsystem history. Keeping both snapshots in review locations creates stable comparison points for later salvage.

Alternative considered:
- Delete the displaced trees after recovery. Rejected because it would discard the clearest evidence of the interrupted refactor and the alternative subsystem snapshot.

### Review build and configuration changes after source restoration
Changes in `Cargo.toml` and `build.rs` appear intentional and may belong in the recovered baseline, but they should be evaluated after the source tree is coherent. This avoids coupling source rollback to unresolved dependency and build-script questions.

Alternative considered:
- Revert all non-`src/` changes immediately. Rejected because some changes, such as generated `VERSION` and `AUTHORS` support, are already known to be valid and useful.

### Keep recovery workflow artifacts tracked
OpenSpec artifacts and project guidance are part of the recovery process and should stay in the repository while cleanup is in progress.

Alternative considered:
- Treat workflow files as temporary and leave them untracked. Rejected for OpenSpec change artifacts and guidance because they document recovery intent and decisions. The local workflow state file `.guide.yaml` remains intentionally ignored.

## Risks / Trade-offs

- [Restored tree is still incomplete] -> Validate the restored repository with compilation and targeted tests before accepting it as the new baseline.
- [Useful refactor changes become stranded] -> Preserve both refactor-era source trees in clearly named side locations and review diffs after baseline recovery.
- [Non-`src/` files drift from the restored code] -> Compare `Cargo.toml`, `build.rs`, and related files against the restored baseline immediately after the move.
- [Recovery becomes irreversible in practice] -> Perform moves rather than destructive deletes and record the chosen locations in the change artifacts.
- [Architecture intent is lost after restoring `HEAD`] -> Document the current built-in plugin architecture, the intended external plugin target, and the role of `../repostats-plugins` before selective salvage begins.

## Migration Plan

1. Preserve the interrupted refactor tree at `../repostats-refactor/src`.
2. Preserve the `src.old/` snapshot as `../repostats-refactor/src.new` after testing it as an alternative recovery candidate.
3. Restore `src/` from `HEAD` as the active working baseline.
4. Remove or relocate test drift that prevents the restored baseline test suite from running.
5. Build and test the restored project to confirm a working baseline.
6. Compare `Cargo.toml`, `build.rs`, and other non-`src/` changes against the restored source tree.
7. Document the current architecture, the built-in plugin model, and the intended external plugin target using `../repostats-plugins` and the preserved refactor as review inputs.
8. Record where the preserved refactor material was placed and any follow-up work needed to merge useful refactor-era improvements back in.

Rollback strategy:
- Restore `src/` from git if the active baseline drifts again.
- Revisit the preserved refactor trees in `../repostats-refactor/` if selective salvage or comparison is needed.

## Open Questions

- Which refactor-era dependency and build-script changes should be retained immediately after the restore versus deferred for later reapplication?
- Which existing docs in `docs/` accurately describe the restored baseline versus the intended future architecture?
- What concrete contract should exist between `repostats` and `../repostats-plugins` when external plugin development becomes first-class?
