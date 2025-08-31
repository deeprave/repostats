# Plugin Functions Documentation

## Overview

The `PluginFunction` system enables plugins to provide multiple command line interfaces and aliases for their functionality. This system bridges the gap between CLI command parsing and plugin activation, allowing users to invoke plugins through various command names and arguments.

## Structure

```rust
pub struct PluginFunction {
    pub name: String,        // Primary function name
    pub description: String, // Function description
    pub aliases: Vec<String>, // Alternative names
}
```

## Core Concepts

### Function Registration

Plugins declare their available functions through the `advertised_functions()` trait method:

```rust
impl Plugin for MyPlugin {
    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![
            PluginFunction {
                name: "analyse".to_string(),
                description: "Analyse repository statistics".to_string(),
                aliases: vec!["analyze".to_string(), "stats".to_string()],
            },
            PluginFunction {
                name: "export".to_string(),
                description: "Export data in various formats".to_string(),
                aliases: vec!["dump".to_string(), "output".to_string()],
            }
        ]
    }
}
```

### Command Line Integration

The function system integrates with CLI parsing through a two-stage process:

#### Stage 1: Command Segmentation

The `CommandSegmenter` splits CLI arguments into discrete command segments:

```bash
repostats --verbose analyse --since 1week export --format json
```

Becomes:
- Global args: `["repostats", "--verbose"]`
- Command segments:
  - `CommandSegment { command_name: "analyse", args: ["--since", "1week"] }`
  - `CommandSegment { command_name: "export", args: ["--format", "json"] }`

#### Stage 2: Function Matching

The plugin manager matches command segments against plugin functions:

1. **Primary Name Matching**: `function.name == segment.command_name`
2. **Alias Matching**: `function.aliases.contains(&segment.command_name)`
3. **Activation**: Creates `ActivePluginInfo` when matched

```rust
// Example matching logic
if function.name == segment.command_name || function.aliases.contains(&segment.command_name) {
    self.active_plugins.push(ActivePluginInfo {
        plugin_name: plugin_name.clone(),
        function_name: function.name.clone(),  // Always uses primary name
        args: segment.args.clone(),           // Command-specific arguments
    });
}
```

## Function Properties

### `name: String`
**Purpose**: Primary identifier for the function within the plugin.

**Requirements**:
- Must be non-empty
- Must not contain control characters
- Should be unique within the plugin
- Used as the canonical function identifier in `ActivePluginInfo`

**Usage**:
- Primary CLI command name
- Function identifier in activation records
- Default command name when no aliases match

**Example**: `"analyse"`, `"export"`, `"generate"`

### `description: String`
**Purpose**: Human-readable description of function capabilities.

**Requirements**:
- Should clearly explain what the function does
- Used in help text and plugin listings
- Must not contain control characters except spaces

**Usage**:
- CLI help documentation
- Plugin function discovery
- User interface displays

**Example**: `"Analyse repository commit patterns and generate statistics"`

### `aliases: Vec<String>`
**Purpose**: Alternative command names that invoke the same function.

**Requirements**:
- Each alias must be unique across all plugin functions
- Should be intuitive synonyms or shorter versions
- Used for user convenience and command compatibility

**Usage**:
- Command line shortcuts
- Alternative spellings (e.g., "analyse"/"analyze")
- Legacy command compatibility

**Benefits**:
- **User Convenience**: Multiple ways to invoke same functionality
- **Internationalisation**: Support different spelling conventions
- **Backwards Compatibility**: Maintain old command names during refactoring

**Example**: `vec!["analyze".to_string(), "stats".to_string(), "report".to_string()]`

## Active Plugin Information

When a function is matched, the system creates an `ActivePluginInfo` record:

```rust
pub struct ActivePluginInfo {
    pub plugin_name: String,   // Name of the matched plugin
    pub function_name: String, // Primary function name (never alias)
    pub args: Vec<String>,     // Arguments from command segment
}
```

### Key Behaviours

1. **Canonical Function Names**: `function_name` always contains the primary name, never an alias
2. **Argument Preservation**: Command arguments are preserved exactly as provided
3. **Plugin Identification**: Links function activation back to source plugin

## Multiple Function Plugins

Plugins can advertise multiple functions for different capabilities:

```rust
fn advertised_functions(&self) -> Vec<PluginFunction> {
    vec![
        PluginFunction {
            name: "start".to_string(),
            description: "Start monitoring repository changes".to_string(),
            aliases: vec!["begin".to_string(), "run".to_string()],
        },
        PluginFunction {
            name: "stop".to_string(),
            description: "Stop monitoring and generate report".to_string(),
            aliases: vec!["end".to_string(), "finish".to_string()],
        },
        PluginFunction {
            name: "status".to_string(),
            description: "Show current monitoring status".to_string(),
            aliases: vec!["info".to_string()],
        }
    ]
}
```

### CLI Usage Examples

```bash
# Using primary names
repostats start --interval 5m
repostats stop --save results.json
repostats status

# Using aliases
repostats run --interval 5m      # Equivalent to 'start'
repostats finish --save data.json # Equivalent to 'stop'
repostats info                    # Equivalent to 'status'
```

## Function Discovery Process

The plugin manager discovers functions through these steps:

1. **Plugin Discovery**: Find all available plugins
2. **Function Collection**: Call `advertised_functions()` on each plugin
3. **Command Registration**: Register all function names and aliases with `CommandSegmenter`
4. **Activation Matching**: Match CLI commands against registered functions during runtime

### Discovery Code Flow

```rust
// Collect all plugin functions
let mut plugin_functions = Vec::new();
for plugin_name in plugin_names {
    if let Some(plugin) = registry.get_plugin(&plugin_name) {
        let functions = plugin.advertised_functions();
        plugin_functions.push((plugin_name, functions));
    }
}

// Build command list for segmenter
let mut known_commands = Vec::new();
for (_, functions) in &plugin_functions {
    for function in functions {
        known_commands.push(function.name.clone());
        known_commands.extend(function.aliases.clone());
    }
}
```

## Best Practices

### Naming Conventions

1. **Primary Names**: Use clear, descriptive verbs
   - ✅ `"analyse"`, `"generate"`, `"export"`
   - ❌ `"do"`, `"thing"`, `"run"`

2. **Aliases**: Provide common variants and shortcuts
   - ✅ `["analyze", "stats", "report"]` for `"analyse"`
   - ❌ `["a", "x", "z"]` (unclear abbreviations)

3. **Consistency**: Maintain consistent naming across functions
   - ✅ `"start"`/`"stop"` pair
   - ❌ `"begin"`/`"terminate"` (inconsistent style)

### Function Design

1. **Single Responsibility**: Each function should have a clear, distinct purpose
2. **Logical Grouping**: Related functions should be in the same plugin
3. **User Experience**: Consider how users will discover and use functions

### Argument Handling

Plugins receive raw command arguments and should:

1. **Validate Arguments**: Check argument count and format
2. **Provide Help**: Handle `--help` or invalid arguments gracefully
3. **Error Reporting**: Return clear error messages for invalid usage

## Integration Examples

### Simple Single-Function Plugin

```rust
impl Plugin for DumpPlugin {
    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![PluginFunction {
            name: "dump".to_string(),
            description: "Dump repository data for debugging".to_string(),
            aliases: vec!["debug".to_string(), "trace".to_string()],
        }]
    }

    async fn execute_function(&self, function_name: &str, args: &[String]) -> Result<()> {
        match function_name {
            "dump" => self.handle_dump(args).await,
            _ => Err(anyhow::anyhow!("Unknown function: {}", function_name)),
        }
    }
}
```

### Multi-Function Plugin with Routing

```rust
impl Plugin for StatisticsPlugin {
    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![
            PluginFunction {
                name: "summary".to_string(),
                description: "Generate statistical summary".to_string(),
                aliases: vec!["stats".to_string()],
            },
            PluginFunction {
                name: "detailed".to_string(),
                description: "Generate detailed statistics report".to_string(),
                aliases: vec!["detail".to_string(), "full".to_string()],
            }
        ]
    }

    async fn execute_function(&self, function_name: &str, args: &[String]) -> Result<()> {
        match function_name {
            "summary" => self.generate_summary(args).await,
            "detailed" => self.generate_detailed(args).await,
            _ => Err(anyhow::anyhow!("Unknown function: {}", function_name)),
        }
    }
}
```

## Error Handling

### Function Not Found

When no plugin function matches a command segment:

```rust
warn!("PluginManager: No plugin found for command '{}'", segment.command_name);
return Err(PluginError::PluginNotFound {
    plugin_name: segment.command_name.clone(),
});
```

### Function Execution Errors

Plugins should handle function execution errors appropriately:

```rust
async fn execute_function(&self, function_name: &str, args: &[String]) -> Result<()> {
    match function_name {
        "analyse" => {
            if args.is_empty() {
                return Err(anyhow::anyhow!("analyse function requires arguments"));
            }
            self.perform_analysis(args).await
        },
        _ => Err(anyhow::anyhow!("Unsupported function: {}", function_name)),
    }
}
```

## See Also

- [PluginInfo Documentation](plugin_info.md)
- [ScanRequires Documentation](scan_requires.md)
- [Command Line Integration Guide](../system_architecture/cli_integration.md)
- [Plugin Author Guide](plugin_author_guide.md)
