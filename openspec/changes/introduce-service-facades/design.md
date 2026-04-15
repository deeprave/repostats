## Context

The current baseline exposes three global subsystem singletons directly through `api.rs` modules:

- `src/notifications/api.rs`
- `src/plugin/api.rs`
- `src/queue/api.rs`

Those modules currently do two jobs at once:

- define the public import surface for each subsystem
- expose the concrete global service instance and its locking/ownership model

That shape creates a few problems in the active baseline:

- call sites depend on raw global-manager access patterns rather than a stable service boundary
- subsystem API modules re-export more surface area than many callers need, which contributes to the current clippy noise around unused public exports
- the locking and initialization details for each subsystem are not expressed consistently
- later changes such as CLI/library separation and external plugin work have to reason about global manager access and subsystem construction at the same time

The preserved refactor direction suggests a cleaner service-wrapper pattern, but the active repository guidance prefers small, reviewable changes rather than replaying broad refactors wholesale. This design therefore focuses on introducing minimal service facade types that standardize access to existing global services without changing subsystem behavior or committing the project to a larger architectural rewrite.

## Goals / Non-Goals

**Goals:**

- introduce consistent facade types for notifications, plugins, and queues
- hide subsystem-specific singleton initialization and borrowing details behind a small service API
- reduce direct exposure of concrete manager construction details from `api.rs`
- create a better boundary for later CLI/library separation work
- enable narrower public APIs without changing runtime behavior

**Non-Goals:**

- changing the runtime ownership model of the underlying managers
- redesigning subsystem internals or their event semantics
- implementing external plugin loading or changing the plugin contract
- moving CLI modules or reshaping the crate’s top-level public API in this change
- solving all current clippy findings as part of the facade introduction alone

## Decisions

### Decision: Introduce thin facade types per subsystem rather than a generic shared abstraction

The change should add a small, explicit facade type for each subsystem:

- `NotificationService`
- `PluginService`
- `QueueService`

Each facade should wrap access to the existing global singleton for that subsystem and expose only the service-oriented operations needed by runtime call sites.

Rationale:

- the three subsystems do not share the same ownership semantics today
- forcing a generic abstraction now would be speculative and risks a YAGNI violation
- explicit facade types keep each subsystem free to preserve its current concurrency model while still presenting a more uniform caller experience

Alternatives considered:

- keep the current `get_*_service()` functions and only trim re-exports
  - rejected because it reduces surface area but does not improve the global-access boundary
- introduce one generic `GlobalService<T>` wrapper
  - rejected because the subsystems do not have a common API shape and the abstraction would mostly wrap locking differences rather than meaningful domain behavior

### Decision: Preserve current manager implementations and wrap them rather than replacing them

The facades should delegate to the existing `AsyncNotificationManager`, `PluginManager`, and `QueueManager` implementations. This change should not replace those managers or move their internal logic.

Rationale:

- the active baseline is already tested and functionally coherent
- replacing managers while introducing facades would combine two kinds of risk in one change
- preserving manager implementations keeps the change focused on API boundaries

Alternatives considered:

- move behavior from the managers into the facade types
  - rejected because it would blur the line between boundary cleanup and subsystem redesign

### Decision: Keep facade operations domain-oriented and avoid exposing raw synchronization primitives

Callers should interact with facades through named operations or focused access points, rather than receiving raw `MutexGuard` or singleton internals wherever possible.

In practice, this means:

- notification-facing code should prefer facade methods that perform subscription or publication work
- plugin-facing code should prefer facade methods that mediate initialization and access to plugin lifecycle operations
- queue-facing code should prefer facade methods that create publishers/consumers or return queue manager access through a stable wrapper

Rationale:

- raw synchronization primitives leak implementation details into callers
- those leaks make later architectural changes more expensive because they spread locking assumptions across the codebase
- narrowing access points aligns with the project goal of making the library surface more reusable and less coupled to top-level application wiring

Alternatives considered:

- expose facade types but continue returning raw manager guards from them
  - partially rejected because it keeps most coupling intact; acceptable only as a temporary compatibility path inside subsystem internals

### Decision: Maintain compatibility through incremental call-site migration

This change should migrate the most direct runtime call sites to the facades first, while preserving enough compatibility to avoid a flag-day rewrite.

A practical migration shape is:

1. add facade types and constructor/access helpers in the existing subsystem API modules
2. update direct runtime call sites to use the facades
3. narrow or relocate broad public re-exports once callers no longer depend on them
4. leave subsystem-internal code unchanged unless required for the facade boundary

Rationale:

- this matches the repository’s recovery guidance to prefer small, reviewable changes
- it reduces the chance of widespread churn across tests and internal modules
- it allows clippy cleanup to happen as a follow-on effect of migration rather than as a risky all-at-once sweep

Alternatives considered:

- convert every call site immediately and fully rewrite API modules
  - rejected as too broad for the current recovery phase

### Decision: Treat public API trimming as a consequence of facade adoption, not an independent redesign

The change should narrow the `api.rs` public surface where the facade work makes that safe, but it should not attempt a full public API redesign in the same step.

Rationale:

- the crate still has unresolved CLI/library boundary work
- aggressively redesigning the public API now would force architectural decisions that the project guidance requires the user to approve explicitly
- the useful near-term outcome is a cleaner boundary, not a final external contract

Alternatives considered:

- fully redesign the public crate surface during this change
  - rejected because it would overlap heavily with `separate-cli-from-library`

### Decision: Treat subsystem `api.rs` files as the stable export boundary

The service facades should become part of the stable library surface, and their primary export location should be the subsystem `api.rs` files rather than `mod.rs` or broad crate-level re-exports.

Concretely:

- `api.rs` owns the stable public boundary for a subsystem
- `mod.rs` remains focused on module wiring and internal organization
- `lib.rs` stays thin and exposes subsystem namespaces rather than serving as the main export dump

This decision is recorded in [ADR-0001](../../adr/ADR-0001-public-api-export-boundaries.md).

Rationale:

- the codebase already treats `api.rs` as the conceptual public boundary for notifications, plugins, and queues
- keeping the boundary in `api.rs` reduces churn relative to moving the contract into `mod.rs`
- a thin `lib.rs` keeps subsystem boundaries visible and avoids flattening the crate prematurely

Alternatives considered:

- make the facades transitional wrappers only
  - rejected because the boundary is already sufficiently clear to treat the facades as part of the stable library interface
- move the stable public boundary into `mod.rs`
  - rejected because it would mix internal module wiring with public API policy
- export the facades broadly from `lib.rs`
  - rejected because it would weaken subsystem boundaries and create unnecessary top-level surface area

### Decision: Start `PluginService` with a narrow stable lifecycle surface

Because `repostats` is intended to be strongly plugin-oriented, the plugin facade should be treated as a durable architectural boundary. However, the initial stable `PluginService` surface should remain narrow and grow only in response to proven caller needs.

In practice, this means:

- expose stable, domain-oriented plugin operations that current runtime orchestration clearly needs
- avoid mirroring the full `PluginManager` method set in the first iteration
- keep manager internals, registry internals, completion tracking, and shutdown coordination details behind internal compatibility paths unless and until they become clearly stable facade concepts

This decision is recorded in [ADR-0002](../../adr/ADR-0002-plugin-facade-lifecycle-boundaries.md).

Rationale:

- plugins are central to the intended architecture, so the facade should be durable rather than transitional
- the current `PluginManager` surface is broader than the likely long-term stable library contract
- starting narrow reduces accidental coupling and keeps future external plugin work from inheriting current implementation details as permanent API commitments

Alternatives considered:

- expose most lifecycle operations directly on the initial `PluginService`
  - rejected because it would freeze too much of the current manager shape into the stable API too early
- treat the plugin facade as purely transitional
  - rejected because the project is intended to be plugin-based and therefore needs a durable plugin boundary

### Decision: Use explicit async facade methods instead of closure-based guarded access

Notification and plugin facades should expose explicit async methods rather than closure-based access to guarded manager state.

For notifications specifically, the stable boundary should expose the event-bus operations and the associated event and enumeration types. For plugins, the stable boundary should remain more curated because plugin lifecycle access is a different architectural concern from event-bus publication and subscription.

This means the subsystem facades should not all be forced into the same shape:

- `NotificationService` should center on event publication, subscription, filtering, and event types
- `PluginService` should center on explicit lifecycle and inspection operations chosen for stability

This decision is recorded in [ADR-0002](../../adr/ADR-0002-plugin-facade-lifecycle-boundaries.md) and [ADR-0003](../../adr/ADR-0003-notification-manager-global-event-bus.md).

Rationale:

- closure-based access keeps the caller mentally attached to the guarded-manager implementation
- explicit async methods create a clearer stable API and avoid spreading locking assumptions
- notifications and plugins play different roles in the architecture and should not be normalized into one generic facade pattern

Alternatives considered:

- closure-based accessors for notification or plugin facades
  - rejected because they do not add enough value at this boundary and would preserve manager-centric coupling

### Decision: Trim exports conservatively but from a minimum-first baseline

This change should start from the minimum export surface needed by real callers, while avoiding a disruptive all-at-once purge of every compatibility re-export.

In practice, this means:

- prefer exposing only the exports needed for active runtime and supported library use cases
- remove broad re-exports when the new facade methods provide a better boundary
- keep temporary compatibility exports only where migration still requires them
- avoid widening the API surface preemptively for convenience

This decision is recorded in [ADR-0001](../../adr/ADR-0001-public-api-export-boundaries.md).

Rationale:

- the codebase still carries historical bleed-through between subsystems and should not preserve that by default
- starting minimal gives the project room to improve implementation details without forcing client churn
- a middle-ground migration keeps the change reviewable while still moving the public boundary in the right direction

Alternatives considered:

- aggressively trim all existing re-exports in this change
  - rejected because it would add unnecessary migration risk and churn to an already cross-cutting change
- preserve broad exports until `separate-cli-from-library`
  - rejected because it would leave the current boundary problems largely intact and defer useful encapsulation gains

## Risks / Trade-offs

- [Facade becomes a pass-through alias with little value] -> Keep the initial facade surface small and domain-oriented so it removes concrete coupling instead of just renaming accessors.
- [Call sites still depend on raw manager guards] -> Allow temporary compatibility only where unavoidable and track remaining raw access in tasks.
- [Public API trimming breaks tests or examples] -> Migrate usage sites first, then narrow exports after consumers are updated.
- [Different subsystem ownership models remain inconsistent] -> Accept that inconsistency for now; the goal is caller-facing consistency, not full internal unification.
- [This change drifts into a broader architectural rewrite] -> Keep CLI separation, plugin contract, and loader implementation explicitly out of scope.

## Migration Plan

1. Document the required facade behavior in the change spec.
2. Add thin facade types and access helpers in `notifications/api.rs`, `plugin/api.rs`, and `queue/api.rs`.
3. Update runtime call sites that currently depend on direct global-manager access.
4. Reduce unnecessary public re-exports that the migrated call sites no longer require.
5. Run targeted tests and clippy to measure whether API-surface warnings are reduced.

Rollback strategy:

- because this change is boundary-focused, rollback is straightforward: revert facade additions and call-site migrations while leaving subsystem internals untouched

## Open Questions

None currently.
