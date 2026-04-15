## ADDED Requirements

### Requirement: Subsystem service facades define the stable public boundary
The library SHALL provide stable subsystem facade entry points for notifications, plugins, and queues through their respective `api.rs` modules.

#### Scenario: Facade types are available from subsystem API modules
- **WHEN** library consumers import the notifications, plugins, or queues public API
- **THEN** they MUST obtain the stable subsystem facade entry points from the subsystem `api.rs` boundary
- **AND** the stable boundary MUST NOT depend on `mod.rs` or broad crate-level re-exporting as the primary export location

#### Scenario: Stable facade location remains subsystem-oriented
- **WHEN** a new stable entry point is added for notifications, plugins, or queues
- **THEN** it MUST be added to the corresponding subsystem `api.rs` module unless a separate architectural decision explicitly overrides that rule

### Requirement: Service facades preserve existing manager behavior behind the boundary
Service facades SHALL delegate to the existing notification, plugin, and queue manager implementations without changing their runtime responsibilities.

#### Scenario: Existing subsystem behavior remains authoritative
- **WHEN** a facade operation invokes notifications, plugins, or queues behavior
- **THEN** the operation MUST delegate to the existing underlying manager implementation
- **AND** the change MUST NOT require replacing those managers as part of facade introduction

#### Scenario: Facade introduction does not redefine subsystem semantics
- **WHEN** the service facades are introduced
- **THEN** event publication, plugin lifecycle handling, and queue management semantics MUST remain consistent with the pre-facade baseline

### Requirement: Facades expose explicit service-oriented methods
Notification and plugin facade access SHALL use explicit async methods rather than closure-based guarded access or raw synchronization primitives as the primary public pattern.

#### Scenario: Notification access avoids closure-based guard exposure
- **WHEN** a caller interacts with the notification public boundary
- **THEN** the stable API MUST provide explicit service-oriented operations for publication, subscription, or filtering
- **AND** the primary public pattern MUST NOT require the caller to supply a closure over a guarded manager

#### Scenario: Plugin access avoids manager-shaped guard exposure
- **WHEN** a caller interacts with the plugin public boundary
- **THEN** the stable API MUST provide explicit lifecycle or inspection operations
- **AND** the primary public pattern MUST NOT expose raw lock guards or closure-based guarded manager access

### Requirement: Notification facade preserves the global event-bus boundary
The notification public boundary SHALL preserve notifications as the application’s global event bus and SHALL expose the associated event and filter types needed for cross-system communication.

#### Scenario: Event-bus types remain part of the stable notification boundary
- **WHEN** callers publish or subscribe to notifications across subsystem boundaries
- **THEN** they MUST be able to access the typed event model and filtering constructs from the notification public boundary

#### Scenario: Notification global access remains intentional
- **WHEN** notification access is refactored behind a facade
- **THEN** the notification subsystem MUST remain globally accessible as cross-system event-bus infrastructure
- **AND** that global role MUST NOT be treated as equivalent to ordinary manager exposure

### Requirement: Plugin facade starts with a narrow stable lifecycle surface
The plugin public boundary SHALL begin with only the lifecycle and inspection operations that are clearly required by current runtime callers and SHALL expand only when concrete caller needs justify broader stable API support.

#### Scenario: Initial plugin facade excludes manager internals
- **WHEN** the initial `PluginService` surface is defined
- **THEN** it MUST NOT expose raw `PluginManager` access, raw registry access, or internal completion-tracking details as part of the stable public contract

#### Scenario: New plugin facade methods require concrete demand
- **WHEN** a new plugin lifecycle operation is proposed for the stable facade
- **THEN** it MUST be justified by a concrete caller need
- **AND** it MUST describe a stable domain concept rather than only mirroring a current manager implementation detail

### Requirement: Export trimming follows a minimum-first migration policy
The service-facade change SHALL reduce subsystem API exports from a minimum-first baseline, keeping only the exports needed by real callers and preserving temporary compatibility exports only where migration still requires them.

#### Scenario: Broad re-exports are removed when better facade methods exist
- **WHEN** a facade method provides a stable subsystem-oriented replacement for a broad public re-export
- **THEN** the broad re-export SHOULD be removed from the stable public boundary for that change

#### Scenario: Compatibility exports remain temporary
- **WHEN** a temporary compatibility export is retained during facade migration
- **THEN** it MUST be justified by an active migration need
- **AND** it MUST NOT be treated as justification for widening the stable API surface by default
