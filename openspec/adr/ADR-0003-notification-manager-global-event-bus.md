# ADR-0003: Notification Manager As Global Event Bus

## Status

Accepted

## Date

2026-04-15

## Context

`repostats` contains several relatively independent subsystems, including scanning, queueing, plugin lifecycle management, application orchestration, and system-level coordination. Those parts need a way to communicate without taking direct dependencies on each other’s concrete implementations.

The existing notification subsystem already fills that role by acting as the cross-system event bus:

- publishers emit typed events such as system, queue, scan, and plugin events
- subscribers register interest through filters
- the manager owns subscription state and fan-out behavior

This is more global than an ordinary service singleton. It is a coordination primitive for the application as a whole, not just a convenience wrapper around one subsystem’s internals.

That global role needs to be made explicit so later facade and library-boundary work does not accidentally treat the notification manager as just another manager-shaped service.

## Decision

The notification manager should be treated as the application’s global event-bus boundary.

Concretely:

- the notification subsystem remains globally accessible because event distribution is inherently cross-cutting in this architecture
- the stable public boundary should center on event publication, subscription, filtering, and the associated event/enumeration types
- notification access should use explicit async methods rather than closure-based guarded access
- the facade should present event-bus semantics first, not generic manager-access semantics

This means the notification boundary is intentionally different from the plugin boundary:

- notifications expose the event bus and its typed event model
- plugins expose a curated lifecycle/service boundary and should not be treated as another generic global manager

## Rationale

- Event-bus behavior is fundamentally global in this application because it enables communication between otherwise independent parts of the system.
- Treating the notification manager as an event-bus boundary clarifies why global access is appropriate there even when other subsystem APIs are narrowed.
- Explicit async methods keep the boundary aligned with event-bus use cases and avoid leaking synchronization details into callers.
- Distinguishing the notification boundary from the plugin boundary avoids forcing two different architectural roles into the same facade pattern.

## Alternatives Considered

### Treat the notification manager like any other narrow subsystem manager

Rejected.

That understates its role as cross-system coordination infrastructure and risks narrowing it in ways that fight the architecture.

### Use closure-based guarded access as the primary notification facade interface

Rejected.

That would preserve raw manager-centric access patterns instead of presenting a clear event-bus boundary.

### Remove global access and require explicit dependency threading everywhere

Rejected for now.

That may become attractive in some future contexts, but the current architecture relies on a shared event bus for decoupled coordination. For the active baseline, global event-bus access remains the clearer fit.

## Consequences

### Positive

- The architectural role of notifications becomes clearer.
- Future facade work can preserve global event-bus access without pretending all subsystem facades should look the same.
- Event and filter types have an obvious stable home alongside the notification boundary.

### Negative

- The notification subsystem will remain intentionally global, which may look inconsistent compared with narrower service boundaries elsewhere.
- Care is still required to avoid turning the event bus into a substitute for explicit domain interfaces where those would be clearer.

## Follow-Up

- Use this ADR to shape `NotificationService` during `introduce-service-facades`.
- Keep event-bus semantics primary when trimming notification exports or refactoring notification access paths.
