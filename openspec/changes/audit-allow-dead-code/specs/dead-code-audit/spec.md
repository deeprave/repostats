## ADDED Requirements

### Requirement: Dead-code allowances require explicit justification
The repository SHALL retain `#[allow(dead_code)]` only for items with a concrete and reviewable justification.

This requirement follows [ADR-0004](../../../../adr/ADR-0004-allow-dead-code-policy.md).

#### Scenario: Unused item has no active justification
- **WHEN** an API, helper, or compatibility item is marked with `#[allow(dead_code)]`
- **AND** it has no current production use, supported test use, documented library contract, or active migration need
- **THEN** the item SHOULD be removed rather than retained behind the allowance

#### Scenario: Remaining allowance is intentionally scoped
- **WHEN** a dead-code allowance is retained after the audit
- **THEN** the allowance MUST be scoped to the narrowest practical item
- **AND** the retained item MUST have a concrete justification that can be explained in review

### Requirement: Speculative API surface must not survive by default
The repository SHALL prefer deleting speculative or obsolete unused APIs over preserving them for hypothetical future use.

#### Scenario: Future-facing helper has no concrete consumer
- **WHEN** an unused API is defended only by possible future usefulness
- **THEN** it MUST NOT be preserved solely on that basis
- **AND** it SHOULD be reintroduced later if a concrete need appears

#### Scenario: Ambiguous unused API affects architecture
- **WHEN** an unused API cannot be clearly classified as justified or removable
- **THEN** the change MUST treat it as an explicit architectural decision point
- **AND** it MUST NOT remain silently justified only by an `#[allow(dead_code)]`

### Requirement: Dead-code audit preserves repository validation health
The dead-code audit SHALL preserve the clean validation baseline established after the facade cleanup.

#### Scenario: Audit removes or narrows allowances
- **WHEN** the audit removes unused items or reduces dead-code suppressions
- **THEN** `cargo clippy --all-targets --all-features -- -D warnings` MUST still pass
- **AND** `cargo test` MUST still pass

#### Scenario: Justified compatibility surface remains
- **WHEN** a compatibility or documented public boundary item is retained
- **THEN** the audit MAY keep a narrowly scoped dead-code allowance for it
- **BUT** the retained allowance MUST NOT hide unrelated dead code in the same module
