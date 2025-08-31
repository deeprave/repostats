# PluginInfo Documentation

## Overview

The `PluginInfo` struct is the central metadata container for all plugins in the repostats system. It provides comprehensive information about a plugin's capabilities, requirements, and interface to the plugin manager.

## Structure

```rust
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub api_version: u32,
    pub plugin_type: PluginType,
    pub functions: Vec<PluginFunction>,
    pub required: u64,
    pub auto_active: bool,
}
```

## Field Descriptions

### `name: String`
**Purpose**: Unique identifier for the plugin within the system.

**Requirements**:
- Must be non-empty
- Must not contain control characters or tabs
- Used for plugin discovery and activation
- Must be unique across all plugins

**Example**: `"dump"`, `"statistics"`, `"export"`

### `version: String`
**Purpose**: Semantic version of the plugin implementation.

**Requirements**:
- Should follow semantic versioning (e.g., "1.0.0")
- Used for compatibility checking
- Displayed in plugin listings

**Example**: `"1.0.0"`, `"2.1.3-beta"`

### `description: String`
**Purpose**: Human-readable description of plugin functionality.

**Requirements**:
- Must not contain control characters (except spaces)
- Should be concise but informative
- Displayed in plugin help and listings

**Example**: `"Dump repository data for debugging purposes"`

### `author: String`
**Purpose**: Plugin author or maintainer identification.

**Requirements**:
- Can be individual name, organization, or email
- Used for plugin attribution
- Displayed in plugin information

**Example**: `"RepoStats"`, `"Jane Smith <jane@example.com>"`

### `api_version: u32`
**Purpose**: API version compatibility indicator.

**Requirements**:
- Must match system API version for plugin to load
- Used for forward/backward compatibility checks
- Current API version: `20250101`

**Example**: `20250101`

### `plugin_type: PluginType`
**Purpose**: Defines functional category and queue interaction behavior.

**Available Types**:
- `PluginType::Processing` - Actively processes queue messages, gets queue subscribers
- `PluginType::Output` - Generates reports/exports, no queue subscribers
- `PluginType::Notification` - Event-driven, no queue subscribers

**Queue Subscriber Rules**:
- **Processing plugins**: Always get queue subscribers for message processing
- **Output plugins**: Do not get queue subscribers
- **Notification plugins**: Do not get queue subscribers

**Example**: `PluginType::Processing`

### `functions: Vec<PluginFunction>`
**Purpose**: List of functions/commands that this plugin provides.

**Structure**:
```rust
pub struct PluginFunction {
    pub name: String,        // Primary function name
    pub description: String, // Function description
    pub aliases: Vec<String>, // Alternative names
}
```

**Requirements**:
- At least one function should be provided for non-auto-active plugins
- Function names must not contain control characters
- Used for CLI command matching and plugin discovery

**Example**:
```rust
vec![PluginFunction {
    name: "dump".to_string(),
    description: "Start dumping messages to stdout".to_string(),
    aliases: vec!["start".to_string(), "run".to_string()],
}]
```

### `required: u64`
**Purpose**: Bitflags indicating what data the plugin requires from the scanner.

**Value Source**: `ScanRequires::bits()` - bitflag representation of requirements

**Available Requirements**:
- `ScanRequires::NONE` (0) - No special requirements
- `ScanRequires::REPOSITORY_INFO` - Repository metadata
- `ScanRequires::HISTORY` - Commit history information
- `ScanRequires::FILE_CHANGES` - File change details
- Combined: `(ScanRequires::REPOSITORY_INFO | ScanRequires::HISTORY).bits()`

**Usage**: Scanner uses this to optimize message generation - only produces messages that active plugins need.

**Example**: `(ScanRequires::REPOSITORY_INFO | ScanRequires::HISTORY | ScanRequires::FILE_CHANGES).bits()`

### `auto_active: bool`
**Purpose**: Whether plugin should be automatically activated during discovery.

**Behavior**:
- `true`: Plugin is activated immediately when discovered, without CLI command
- `false`: Plugin only activated when explicitly requested via CLI command

**Use Cases**:
- Background monitoring plugins
- System health checkers
- Automatic data collection plugins

**Default**: `false` (most plugins should be explicitly activated)

## Implementation Example

```rust
fn plugin_info(&self) -> PluginInfo {
    PluginInfo {
        name: "example".to_string(),
        version: "1.0.0".to_string(),
        description: "Example plugin for demonstration".to_string(),
        author: "Plugin Developer".to_string(),
        api_version: 20250101,
        plugin_type: PluginType::Processing,
        functions: vec![
            PluginFunction {
                name: "process".to_string(),
                description: "Process repository data".to_string(),
                aliases: vec!["proc".to_string(), "run".to_string()],
            }
        ],
        required: (ScanRequires::REPOSITORY_INFO | ScanRequires::FILE_CHANGES).bits(),
        auto_active: false,
    }
}
```

## Validation Rules

The system validates `PluginInfo` during plugin registration:

1. **Name validation**: Non-empty, no control characters
2. **Description validation**: No control characters except spaces
3. **Function validation**: All function names free of control characters
4. **API compatibility**: `api_version` must match system version
5. **Type consistency**: `plugin_type` determines queue subscriber allocation

## Integration with Plugin Traits

`PluginInfo` should be consistent with plugin trait method implementations:

```rust
impl Plugin for MyPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            // ... other fields ...
            plugin_type: self.plugin_type(),           // Should match
            functions: self.advertised_functions(),     // Should match
            required: self.requirements().bits(),      // Should match
            // ...
        }
    }

    fn plugin_type(&self) -> PluginType { /* ... */ }
    fn advertised_functions(&self) -> Vec<PluginFunction> { /* ... */ }
    fn requirements(&self) -> ScanRequires { /* ... */ }
}
```

## See Also

- [ScanRequires Documentation](scan_requires.md)
- [Plugin Functions Documentation](plugin_functions.md)
- [Plugin Author Guide](plugin_author_guide.md)
