## ADDED Requirements

### Requirement: Repository can be restored to a compilable baseline
The repository SHALL provide a recovery procedure that re-establishes a coherent `src/` tree from the last known working source layout before any further refactor work is resumed.

#### Scenario: Restore working source layout
- **WHEN** the repository contains an interrupted refactor and a preserved fallback source snapshot
- **THEN** the recovery procedure restores the fallback source snapshot into `src/`
- **AND** the repository has a single primary source tree that can be evaluated for compilation

### Requirement: Interrupted refactor state is preserved for later review
The recovery procedure SHALL preserve the in-progress refactor source tree in a separate location so its code and intent remain available for later comparison and selective reapplication.

#### Scenario: Preserve refactor tree before restore
- **WHEN** recovery begins from a repository containing both an interrupted refactor tree and a fallback source snapshot
- **THEN** the interrupted refactor tree is moved to a separate review location before the fallback source tree is restored
- **AND** no refactor-only files are discarded as part of the restore step

### Requirement: Non-source refactor changes are reviewed after source restoration
The recovery procedure SHALL evaluate build and configuration changes separately from source-tree restoration so intentional improvements can be retained without blocking the rollback to a working baseline.

#### Scenario: Review build changes after source restore
- **WHEN** source restoration is complete
- **THEN** build-related files such as `Cargo.toml` and `build.rs` are compared against the restored baseline
- **AND** intentional changes are identified for selective retention or reapplication

### Requirement: Recovery workflow documents the repository state transition
The recovery change SHALL record the source of the restored baseline, the location of the preserved refactor tree, and any follow-up decisions needed to finish cleanup.

#### Scenario: Record recovery outcomes
- **WHEN** recovery steps are executed
- **THEN** the change artifacts document which tree became the restored `src/`
- **AND** the change artifacts document where the interrupted refactor was preserved
- **AND** the change artifacts identify unresolved follow-up review items

### Requirement: Current baseline architecture is documented
The recovery change SHALL document the major modules and subsystem relationships in the restored baseline so future refactor work starts from an accurate description of the current working system.

#### Scenario: Inventory current baseline architecture
- **WHEN** the repository has been restored to a working baseline
- **THEN** the change artifacts identify the key modules and subsystems in the current source tree
- **AND** the change artifacts describe the main relationships between those components

### Requirement: Plugin model transition is documented
The recovery change SHALL document both the current built-in plugin model and the intended transition to externally developed dynamically loaded plugins.

#### Scenario: Describe plugin transition target
- **WHEN** the preserved refactor is reviewed in the context of the restored baseline
- **THEN** the change artifacts describe how the current built-in plugin approach works
- **AND** the change artifacts describe the intended external plugin development model
- **AND** the change artifacts identify `../repostats-plugins` as a source of context for the intended external plugin workflow
