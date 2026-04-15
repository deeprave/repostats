## ADDED Requirements

### Requirement: ScannerTask initialization distinguishes required and optional inputs
`ScannerTask` initialization SHALL distinguish between required runtime dependencies and optional or configuration-driven state.

#### Scenario: Required runtime dependencies remain explicit
- **WHEN** a scanner task is created for normal scanner execution
- **THEN** the initialization API MUST require all dependencies that are fundamental to publishing scanner output

#### Scenario: Optional scanner state is not required in the core constructor
- **WHEN** a scanner task is created without optional configuration features
- **THEN** the core initialization path MUST NOT require optional or configuration-driven inputs to be passed as flat constructor arguments

### Requirement: QueuePublisher remains required for scanner execution
`QueuePublisher` SHALL remain a required scanner task dependency because normal scanner execution publishes scan output to the queue.

#### Scenario: Scanner task publishes normal scan output
- **WHEN** a scanner task runs normal scan operations and emits `ScanMessage` values
- **THEN** it MUST use its configured `QueuePublisher` to publish those messages

#### Scenario: Scanner task cannot be created without queue publishing capability
- **WHEN** scanner runtime code constructs a scanner task for normal use
- **THEN** the initialization contract MUST require a `QueuePublisher`

### Requirement: Notification manager injection supports a default runtime path
`ScannerTask` initialization SHALL support explicit notification manager injection for tests or specialized callers, and SHALL support a default runtime path that obtains the notification manager from global service state.

#### Scenario: Runtime construction omits notification manager override
- **WHEN** normal runtime code creates a scanner task without a custom notification manager
- **THEN** scanner initialization MUST use the global notification service as the default source

#### Scenario: Tests override notification manager explicitly
- **WHEN** tests or specialized callers provide a custom notification manager
- **THEN** scanner initialization MUST use the provided notification manager instead of the global default

### Requirement: Optional scanner configuration uses builder-style initialization
Optional or configuration-driven scanner state such as requirements, query parameters, and checkout manager SHALL be configured through builder or `with_*` style initialization rather than a long flat constructor signature.

#### Scenario: Scanner task sets query and requirement configuration
- **WHEN** scanner initialization needs to apply requirements or query parameters
- **THEN** those values MUST be set through the builder-style or equivalent configuration flow

#### Scenario: Checkout manager is only supplied when checkout is needed
- **WHEN** scanner execution requires checkout support
- **THEN** initialization MUST allow a checkout manager to be supplied through the optional configuration path

#### Scenario: Scanner task can be built without checkout support
- **WHEN** scanner execution does not require repository checkout support
- **THEN** scanner initialization MUST succeed without an explicit checkout manager
