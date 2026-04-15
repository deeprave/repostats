## 1. Define stable subsystem facade boundaries

- [x] 1.1 Add `NotificationService` to `src/notifications/api.rs` as the stable notification facade entry point.
- [x] 1.2 Add `PluginService` to `src/plugin/api.rs` as the stable plugin facade entry point with a deliberately narrow lifecycle and inspection surface.
- [x] 1.3 Add `QueueService` to `src/queue/api.rs` as the stable queue facade entry point.
- [x] 1.4 Ensure notifications, plugins, and queues continue to expose their stable public boundaries primarily through `api.rs` rather than `mod.rs` or broad crate-level re-exports.

## 2. Implement facade access patterns

- [x] 2.1 Implement explicit async notification facade methods for publication, subscription, and filtering operations without closure-based guarded manager access.
- [x] 2.2 Implement explicit async plugin facade methods for the initial stable lifecycle and inspection operations without exposing raw `PluginManager` or registry internals.
- [x] 2.3 Implement queue facade methods that provide stable publisher/consumer or manager-oriented operations without widening the public API unnecessarily.
- [x] 2.4 Keep the facades delegating to the existing manager implementations so runtime behavior remains unchanged.

## 3. Migrate callers and reduce boundary leakage

- [x] 3.1 Update the most direct runtime call sites to use the new notification, plugin, and queue facades instead of raw global-manager access patterns.
- [x] 3.2 Retain only the temporary compatibility exports needed for active migration paths.
- [x] 3.3 Remove broad public re-exports that are superseded by the facade methods, starting from a minimum-first export baseline.
- [x] 3.4 Keep notification’s global event-bus role explicit while preserving a more curated stable boundary for plugin lifecycle access.

## 4. Validate boundary behavior

- [x] 4.1 Update or add tests that exercise the new facade entry points and confirm subsystem behavior is unchanged.
- [x] 4.2 Run targeted notifications, plugin, and queue tests to verify facade delegation and event-bus behavior remain coherent.
- [x] 4.3 Run `cargo test` to confirm the working baseline remains intact after the facade migration.
- [x] 4.4 Run `cargo clippy --all-targets --all-features -- -D warnings` and capture which API-surface warnings are resolved or still deferred.

## Notes

- `cargo test` now passes, including doctests.
- `cargo clippy --all-targets --all-features -- -D warnings` now passes.
- This change eliminated the typed-queue dead-code and compatibility-helper warnings that were directly surfaced by the facade migration work.
- Additional export cleanup completed in this change:
  - removed broad plugin data-export re-exports from `src/plugin/api.rs`
  - removed `ScanStats` from `src/scanner/api.rs`
  - kept a small set of intentional public/testing-facing re-exports in `src/queue/api.rs` and `src/scanner/api.rs`, with explicit annotations where `clippy` would otherwise misclassify them as unused
- Plugin lifecycle boundary cleanup completed in this change:
  - removed raw registry access from the stable `PluginService` surface
  - updated runtime output-plugin code to query active plugins through explicit facade methods instead of a registry handle
  - kept notifications as the explicit cross-system event-bus boundary through `NotificationService`
- Additional `4.4` progress in this change:
  - removed redundant single-component imports flagged by clippy
  - fixed test-side `clippy` complaints in scanner and CLI validation tests
  - marked intentional compatibility and future-facing helper APIs explicitly where they remain part of the stable or planned boundary but are not yet exercised by current runtime paths
  - cleared the remaining dead-code backlog across `core`, `plugin`, `queue`, and `scanner` that blocked `clippy -D warnings`
