# ScanRequires Documentation

## Overview

`ScanRequires` is a bitflag system that allows plugins to declare what data they need from the scanner. The scanner uses these requirements to optimize message generation and only produces the data that active plugins actually need.

## Structure

`ScanRequires` is implemented as a wrapper around `u64` with bitflag operations:

```rust
pub struct ScanRequires(u64);
```

## Available Requirements

### `ScanRequires::NONE`
**Value**: `0`
**Purpose**: Plugin requires no specific data from scanner
**Usage**: Default for plugins that work with minimal data

### `ScanRequires::REPOSITORY_INFO`
**Value**: `1 << 0` (bit 0)
**Purpose**: Repository metadata (name, path, branch, etc.)
**Dependencies**: None
**Use Cases**: Plugins that need basic repository information

### `ScanRequires::COMMITS`
**Value**: `1 << 1` (bit 1)
**Purpose**: Individual commit information
**Dependencies**: None
**Use Cases**: Plugins analyzing commit patterns, authors, timestamps

### `ScanRequires::FILE_CHANGES`
**Value**: `1 << 2 | COMMITS` (bit 2 + dependencies)
**Purpose**: Detailed file change information for each commit
**Dependencies**: Automatically includes `COMMITS`
**Use Cases**: Plugins analyzing code changes, file modifications

### `ScanRequires::FILE_CONTENT`
**Value**: `1 << 3 | FILE_CHANGES` (bit 3 + dependencies)
**Purpose**: Actual file content at HEAD/tag/commit
**Dependencies**: Automatically includes `FILE_CHANGES` and `COMMITS`
**Use Cases**: Static analysis, code quality checking, content processing

### `ScanRequires::HISTORY`
**Value**: `1 << 4 | COMMITS` (bit 4 + dependencies)
**Purpose**: Full commit history traversal
**Dependencies**: Automatically includes `COMMITS`
**Use Cases**: Historical analysis, trend detection, long-term statistics

## Dependency Rules

Requirements automatically include their dependencies:

```
FILE_CONTENT
    └── FILE_CHANGES
        └── COMMITS

HISTORY
    └── COMMITS

REPOSITORY_INFO (independent)
```

**Important**: You only need to specify the highest-level requirement you need. Dependencies are automatically included.

## Additive Behavior

The plugin system uses **additive requirements aggregation**:

- If ANY active plugin needs a requirement, ALL active plugins receive those messages
- Individual plugins do NOT get filtered message streams
- Plugins must filter messages themselves if they're not interested
- This prevents complex per-plugin message routing

**Example**: If Plugin A needs `COMMITS` and Plugin B needs `FILE_CHANGES`, both plugins will receive `FILE_CHANGES` messages (which include `COMMITS`).

## API Methods

### Construction and Conversion
```rust
ScanRequires::NONE                    // No requirements
ScanRequires::from_bits(42)          // From raw bits
requirements.bits()                   // To raw bits (for PluginInfo.required)
```

### Combining Requirements
```rust
let combined = ScanRequires::REPOSITORY_INFO | ScanRequires::FILE_CHANGES;
let mut reqs = ScanRequires::COMMITS;
reqs |= ScanRequires::HISTORY;       // Add more requirements
```

### Checking Requirements
```rust
requirements.is_empty()                           // No requirements set
requirements.contains(ScanRequires::COMMITS)     // Has specific requirement
requirements.requires_repository_info()          // Convenience methods
requirements.requires_commits()
requirements.requires_file_changes()
requirements.requires_file_content()
requirements.requires_history()
```

### Set Operations
```rust
let union = req1.union(req2);              // Combine (OR)
let intersection = req1.intersection(req2); // Common (AND)
let difference = req1.difference(req2);     // Remove
```

## Usage in Plugins

### Plugin Trait Implementation
```rust
impl Plugin for MyPlugin {
    fn requirements(&self) -> ScanRequires {
        // Specify the highest level you need - dependencies included automatically
        ScanRequires::FILE_CHANGES | ScanRequires::REPOSITORY_INFO
    }
}
```

### PluginInfo Integration
```rust
fn plugin_info(&self) -> PluginInfo {
    PluginInfo {
        // Convert to bits for storage
        required: self.requirements().bits(),
        // ... other fields
    }
}
```

## Common Patterns

### Basic Repository Analysis
```rust
// Only need repository metadata
ScanRequires::REPOSITORY_INFO
```

### Commit Statistics
```rust
// Need commit information but not file details
ScanRequires::COMMITS
```

### Code Change Analysis
```rust
// Need file changes (automatically includes commits)
ScanRequires::FILE_CHANGES
```

### Static Code Analysis
```rust
// Need actual file content (includes file changes and commits)
ScanRequires::FILE_CONTENT
```

### Historical Trend Analysis
```rust
// Need full history traversal and repository info
ScanRequires::HISTORY | ScanRequires::REPOSITORY_INFO
```

### Comprehensive Analysis
```rust
// Need everything
ScanRequires::FILE_CONTENT | ScanRequires::HISTORY | ScanRequires::REPOSITORY_INFO
```

## Performance Considerations

### Scanner Optimization
- Scanner only generates messages for required data types
- Unused message types are completely skipped
- Significant performance improvement when plugins need minimal data

### Memory Usage
- File content requirements can use substantial memory for large repositories
- History traversal can be time-intensive for repositories with many commits
- Consider whether your plugin truly needs high-level requirements

### Dependency Inclusion
- Requesting `FILE_CONTENT` automatically includes `FILE_CHANGES` and `COMMITS`
- Don't specify dependencies explicitly - they're included automatically
- Specifying lower-level dependencies redundantly doesn't hurt but isn't necessary

## Examples

### Example 1: Statistics Plugin
```rust
impl Plugin for StatisticsPlugin {
    fn requirements(&self) -> ScanRequires {
        // Need commit info and repository metadata for statistics
        ScanRequires::COMMITS | ScanRequires::REPOSITORY_INFO
    }
}
```

### Example 2: Code Quality Plugin
```rust
impl Plugin for CodeQualityPlugin {
    fn requirements(&self) -> ScanRequires {
        // Need actual file content for static analysis
        // This automatically includes FILE_CHANGES and COMMITS
        ScanRequires::FILE_CONTENT
    }
}
```

### Example 3: History Analyzer Plugin
```rust
impl Plugin for HistoryAnalyzerPlugin {
    fn requirements(&self) -> ScanRequires {
        // Need full history and repository info
        // HISTORY automatically includes COMMITS
        ScanRequires::HISTORY | ScanRequires::REPOSITORY_INFO
    }
}
```

## Display Format

`ScanRequires` implements `Display` for human-readable output:

```rust
println!("{}", requirements);
// Output examples:
// "None"
// "RepositoryInfo"
// "RepositoryInfo, FileChanges"
// "RepositoryInfo, FileContent, History"
```

## Integration with Scanner

The plugin manager collects requirements from all active plugins and passes the combined requirements to the scanner:

```rust
// Plugin manager aggregates requirements
let combined_requirements = plugin_manager.get_combined_requirements();

// Scanner uses requirements to optimize message generation
scanner.set_requirements(combined_requirements);
```

## See Also

- [PluginInfo Documentation](plugin_info.md)
- [Plugin Functions Documentation](plugin_functions.md)
- [Scanner Integration Documentation](../system_architecture/scanner_integration.md)
