# ADR-0001: Public API Export Boundaries

## Status

Accepted

## Date

2026-04-15

## Context

`repostats` is moving toward a stable library interface built around explicit subsystem boundaries. The current codebase already treats `api.rs` as the conceptual public boundary for several subsystems, especially:

- `src/notifications/api.rs`
- `src/plugin/api.rs`
- `src/queue/api.rs`

At the same time, the crate still has unresolved CLI/library separation work, and the current public surface is broader than necessary. The project needs a clear rule for where exports should live so service-facade work can proceed without repeatedly revisiting module-boundary policy.

## Decision

Public exports should follow this structure:

- `api.rs`
  - owns the stable public boundary for a subsystem
  - defines and exports facade types and other supported subsystem entry points
- `mod.rs`
  - owns module wiring and internal organization
  - should avoid acting as the primary public contract for the subsystem
- `lib.rs`
  - stays thin
  - exposes subsystem namespaces and only a very small set of crate-level convenience re-exports when they are truly canonical

For the current repository, this means the stable public interface for notifications, plugins, and queues should live primarily in:

- `src/notifications/api.rs`
- `src/plugin/api.rs`
- `src/queue/api.rs`

Export breadth should follow a minimum-first policy:

- start with the minimum export surface required by real callers
- add exports only when a concrete use case demonstrates that they belong in the stable API
- prefer replacing broad re-exports with narrower, better-shaped facade methods over time
- treat encapsulation as the default so subsystem internals can continue to change without unnecessarily affecting clients

## Rationale

- The repo already uses `api.rs` as the conceptual boundary, so continuing that pattern minimizes churn.
- Keeping `mod.rs` focused on internal structure avoids mixing module assembly with public API policy.
- Keeping `lib.rs` thin prevents it from becoming a dumping ground for broad re-exports.
- This gives each subsystem one obvious public boundary file, which supports the goal of a stable library interface.
- The rule is specific enough to guide the service-facades change, but narrow enough to avoid prematurely redesigning the entire crate surface.
- The codebase still carries historical subsystem bleed-through, so a minimum-first export policy is safer than preserving broad compatibility exports indefinitely.
- Strong encapsulation preserves freedom to improve underlying implementations without forcing client-facing churn.

## Alternatives Considered

### Use `mod.rs` as the public boundary and remove `api.rs`

Rejected.

This is structurally simpler, but it adds churn to a codebase that already treats `api.rs` as the boundary. It also makes `mod.rs` responsible for both internal assembly and public interface policy.

### Export broadly from `lib.rs`

Rejected.

This would flatten the crate too aggressively, make the public surface harder to reason about, and work against the goal of preserving subsystem boundaries.

### Preserve broad compatibility re-exports until a later cleanup change

Rejected as the default policy.

Some temporary compatibility re-exports may still be needed during migration, but keeping exports broad by default would preserve the same unclear subsystem boundaries this recovery effort is trying to clean up.

## Consequences

### Positive

- Subsystem public boundaries become more predictable.
- Service facades have a clear home.
- Later CLI/library separation can build on stable subsystem interfaces instead of broad crate-level exports.
- Encapsulation is strengthened, which makes future internal refactors safer.

### Negative

- Some existing exports may need to move or be trimmed, which can create incremental migration work.
- A few compatibility re-exports may still be needed temporarily while callers migrate.
- Some previously convenient imports may no longer be available until better-shaped stable methods are introduced.

## Follow-Up

- Use this export-boundary rule in `introduce-service-facades`.
- Reference this ADR when deciding where new stable subsystem entry points should live.
- Prefer adding stable exports only after a concrete caller demonstrates the need.
