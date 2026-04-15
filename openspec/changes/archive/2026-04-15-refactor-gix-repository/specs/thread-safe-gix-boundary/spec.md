## ADDED Requirements

### Requirement: Shared scanner state uses thread-safe repository handles
The scanner subsystem SHALL store repositories in shared scanner state using `gix::ThreadSafeRepository` rather than `gix::Repository`.

#### Scenario: Shared scanner task stores repository safely
- **WHEN** a `ScannerTask` is created for long-lived shared use
- **THEN** the stored repository handle MUST be a `gix::ThreadSafeRepository`

#### Scenario: Shared scanner manager stores scanner tasks safely
- **WHEN** `ScannerManager` stores scanner tasks in shared state
- **THEN** the shared task graph MUST NOT require `gix::Repository` to remain in long-lived shared storage

### Requirement: Repository validation occurs before shared conversion
The scanner subsystem SHALL perform repository discovery and immediate validation using a thread-local `gix::Repository`, and SHALL convert the repository to `gix::ThreadSafeRepository` only before storing it in shared scanner state.

#### Scenario: Repository validation uses thread-local handle
- **WHEN** a repository is discovered and validated during scanner setup
- **THEN** git reference validation MUST run against a thread-local `gix::Repository`

#### Scenario: Shared storage conversion occurs after validation
- **WHEN** repository validation succeeds and the scanner is ready to retain repository state
- **THEN** the repository MUST be converted to `gix::ThreadSafeRepository` before being stored in shared scanner objects

### Requirement: Git operations materialize thread-local repositories at execution boundaries
Scanner git operations SHALL obtain a thread-local `gix::Repository` from the stored `gix::ThreadSafeRepository` at operation boundaries and MAY continue using existing helper functions that accept `&gix::Repository`.

#### Scenario: Scanner task begins a git operation
- **WHEN** a scanner task enters a git operation path such as traversal, diff analysis, or file extraction
- **THEN** it MUST materialize a thread-local `gix::Repository` for that operation

#### Scenario: Existing helper functions remain repository-based
- **WHEN** scanner git helper functions are invoked after repository materialization
- **THEN** they MAY continue to receive `&gix::Repository` without requiring the shared repository type in their signatures

### Requirement: Shared scanner API exposes repository access through the new boundary
The scanner task API SHALL replace direct long-lived borrowing of the stored repository with a boundary helper or accessor pattern that makes thread-local materialization explicit.

#### Scenario: Shared repository accessor is no longer a borrowed field reference
- **WHEN** scanner code needs repository access from a shared `ScannerTask`
- **THEN** it MUST use the repository boundary helper or accessor rather than borrowing a stored `&gix::Repository` directly

#### Scenario: Repository boundary remains internal to scanner implementation
- **WHEN** scanner internals are refactored to use `gix::ThreadSafeRepository`
- **THEN** the change MUST remain internal to scanner implementation details and MUST NOT require replacing the existing `gix` crate family
