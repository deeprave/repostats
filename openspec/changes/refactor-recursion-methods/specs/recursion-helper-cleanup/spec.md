## ADDED Requirements

### Requirement: Recursive scanner helpers avoid unused instance parameters
Private recursive scanner helper logic SHALL be structured so recursion does not depend on an unused `self` parameter when no instance state is required.

#### Scenario: Recursive tree counting helper has no instance dependency
- **WHEN** scanner code counts entries in a git tree recursively
- **THEN** the recursive helper MUST NOT require `&self` unless it uses scanner task instance state

#### Scenario: Recursive tree extraction helper has no instance dependency
- **WHEN** scanner code recursively extracts tree contents to a target directory
- **THEN** the recursive helper MUST NOT require `&self` unless it uses scanner task instance state

### Requirement: Recursive helper refactor preserves behavior
Refactoring recursive scanner helpers to remove unused instance parameters SHALL preserve existing scanner behavior.

#### Scenario: Tree counting behavior is preserved
- **WHEN** recursive tree counting runs after the refactor
- **THEN** it MUST produce the same entry counts as before

#### Scenario: Tree extraction behavior is preserved
- **WHEN** recursive tree extraction runs after the refactor
- **THEN** it MUST create the same extracted file structure and progress semantics as before
