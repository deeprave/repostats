# ADR-0002: Plugin Facade Lifecycle Boundaries

## Status

Accepted

## Date

2026-04-15

## Context

`repostats` is intended to be heavily plugin-based. Even where current functionality is still anchored in built-in plugins and a global `PluginManager`, the long-term architecture depends on plugins as a primary extension and behavior mechanism.

That makes plugin-boundary decisions more durable than ordinary wrapper decisions. The project needs a clear rule for:

- how much plugin lifecycle surface becomes part of the stable library API now
- which lifecycle responsibilities remain internal to `PluginManager`
- how the facade should evolve as plugin needs become clearer

The current `PluginManager` already owns a broad set of responsibilities, including:

- initialization and event-subscription setup
- plugin discovery and compatibility handling
- registry ownership
- active-plugin inspection
- plugin shutdown and completion coordination
- notification-manager integration
- plugin-specific configuration handling

Exposing that full surface immediately through a stable `PluginService` facade would bind the library API to a manager shape that is still likely to evolve. At the same time, keeping plugin access entirely ad hoc would undermine the goal of a stable plugin-oriented library interface.

## Decision

`PluginService` should begin as a narrow stable facade, and expand only when concrete caller needs justify the additional API surface.

The stable facade should cover only the plugin lifecycle operations that are clearly required by current runtime callers and that are likely to remain conceptually stable across future plugin work.

Broadly:

- stable facade surface
  - should expose a small set of domain-oriented plugin operations
  - should avoid exposing raw manager internals, raw registry access, or synchronization primitives
  - should prefer explicit async methods that describe intent rather than mirror manager implementation structure
- internal compatibility surface
  - may continue to exist temporarily for operations that current internal code still needs
  - should not be treated as part of the stable library contract
  - should shrink over time as concrete facade methods are added

This ADR applies to plugin lifecycle and facade decisions generally, not only to the initial `introduce-service-facades` change.

## Initial Plugin Facade Policy

The initial stable `PluginService` surface should bias toward:

- service initialization
- plugin listing and inspection
- stable read-oriented queries about plugin state
- narrow lifecycle entry points that current runtime orchestration clearly requires

The initial stable surface should avoid:

- direct exposure of `PluginManager`
- direct exposure of `SharedPluginRegistry`
- raw lock guards or closure-based lock access as the primary public pattern
- broad shutdown coordination internals
- internal completion-tracking details
- configuration internals that are not yet part of a settled external contract

When in doubt, do not add a facade method until there is a concrete caller that needs it and the operation reads as a stable domain concept rather than a manager implementation detail.

## Rationale

- Plugins are central to the intended architecture, so the facade must be stable enough to matter.
- The current manager surface is broader than the likely long-term public contract.
- Starting narrow reduces accidental coupling to current internals.
- Expansion based on proven need is consistent with YAGNI and lowers the cost of future architectural refinement.
- A domain-oriented facade makes it easier to support future external plugin work without promising every current manager method forever.

## Alternatives Considered

### Expose most or all `PluginManager` lifecycle operations directly through `PluginService`

Rejected.

This would be convenient in the short term, but it would effectively freeze the current manager shape into the public API before the plugin architecture is fully settled.

### Keep `PluginService` purely transitional and avoid treating it as a stable plugin boundary

Rejected.

Because the application is intended to be plugin-driven, the plugin facade is too central to treat as a disposable wrapper. It should be designed as a durable boundary, even if it starts small.

### Use raw guarded-manager access as the primary plugin facade interface

Rejected.

That would preserve implementation leakage and spread locking assumptions into callers, making later evolution harder.

### Use closure-based guarded access as the primary plugin facade interface

Rejected.

At this boundary, closure-based access does not add enough value to justify exposing the guarded-manager model. Explicit async methods describe plugin lifecycle intent more clearly and keep the stable API smaller.

## Consequences

### Positive

- The plugin public API stays smaller and easier to reason about.
- Future plugin-contract and external-loader work can build on a stable, intentional boundary.
- The project can add facade methods in response to real needs instead of speculative completeness.

### Negative

- Some internal code may need temporary compatibility paths while the stable facade is still narrow.
- New facade methods may need to be added incrementally as more plugin workflows are formalized.

## Guidance For Future Decisions

When deciding whether a plugin lifecycle operation belongs on `PluginService`, ask:

1. Is there a concrete caller that needs this now?
2. Does the operation describe a stable domain concept, or just a current manager detail?
3. Can it be expressed without exposing raw synchronization or registry internals?
4. Would we be comfortable supporting this shape as part of the stable library API?

If any answer is unclear, prefer keeping the operation behind an internal compatibility path until the need is clearer.

## Follow-Up

- Use this ADR to scope the initial `PluginService` facade in `introduce-service-facades`.
- Record explicit additions to the stable plugin facade when later changes broaden the supported lifecycle surface.
