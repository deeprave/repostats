## 1. Preserve and restore source trees

- [x] 1.1 Choose and create the side location that will hold the interrupted refactor source tree.
- [x] 1.2 Move the current `src/` tree to the chosen side location without deleting any refactor files.
- [x] 1.3 Test `src.old/` as an alternative restore candidate and preserve it in a review location.
- [x] 1.4 Restore `src/` from `HEAD` as the active working baseline and record preserved refactor locations in project context.

## 2. Review non-source changes

- [x] 2.1 Compare `Cargo.toml` and `build.rs` against `HEAD` after the source restore.
- [x] 2.2 Identify which refactor-era build and dependency changes are intentional and should remain in the recovered baseline.
- [x] 2.3 Review other non-`src/` additions needed for recovery work, including `openspec/` and project guidance files, and keep them tracked.

## 3. Validate restored baseline

- [x] 3.1 Build the restored project and capture compile errors or warnings that still block a working baseline.
- [x] 3.2 Remove or relocate known test drift that prevents the restored baseline from running its test suite.
- [x] 3.3 Run targeted tests or verification steps to confirm the recovered baseline behaves coherently enough for follow-up refactor work.

## 4. Document current architecture and target direction

- [x] 4.1 Identify the key modules and subsystems in the restored `HEAD` source tree.
- [x] 4.2 Identify the main relationships between the app, core, notifications, plugin, queue, and scanner components.
- [x] 4.3 Review existing `docs/` content and classify which documents describe the current baseline accurately, which describe intended direction, and which need refresh.
- [x] 4.4 Review `../repostats-plugins` as the intended external plugin development project and capture what contract it implies for the future `repostats` library.

## 5. Prepare follow-up recovery analysis

- [ ] 5.1 Diff the preserved refactor trees against the restored baseline to identify intentional improvements worth reapplying later.
- [ ] 5.2 Document unresolved decisions about dependency changes, build-script updates, built-in versus external plugin boundaries, and dynamic loading work that should be handled in later changes.
