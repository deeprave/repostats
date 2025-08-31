# Command Vector and CLI Flag Parsing Documentation

## Overview

The CLI integration system provides a sophisticated command-line interface for plugins through a multi-stage process: command segmentation, plugin activation, and argument parsing. This allows plugins to receive structured command arguments and configuration while maintaining isolation and flexibility.

## Architecture

### Three-Stage CLI Processing

1. **Command Segmentation**: Raw CLI arguments are split into global and plugin-specific segments
2. **Plugin Activation**: Command segments are matched against plugin functions to activate plugins
3. **Argument Parsing**: Individual plugins parse their specific arguments using configurable parsers

## Command Segmentation

### CommandSegment Structure

CLI arguments are organised into command segments that represent discrete plugin invocations:

```rust
pub struct CommandSegment {
    pub command_name: String,  // Matched plugin function name or alias
    pub args: Vec<String>,     // Arguments specific to this command
}
```

### Segmentation Process

The `CommandSegmenter` splits CLI arguments by command boundaries:

```rust
// Example CLI input
repostats --verbose --config app.toml analyse --since 1week export --format json

// Becomes:
// Global args: ["repostats", "--verbose", "--config", "app.toml"]
// Command segments:
[
    CommandSegment {
        command_name: "analyse",
        args: ["--since", "1week"]
    },
    CommandSegment {
        command_name: "export",
        args: ["--format", "json"]
    }
]
```

### Command Discovery

The segmenter maintains a registry of known commands built from plugin functions:

```rust
// Collected from all plugin advertised_functions()
let known_commands = vec![
    "analyse",     // Primary function name
    "analyze",     // Function alias
    "stats",       // Function alias
    "export",      // Different function
    "dump",        // Function alias
];
```

### Segmentation Algorithm

1. **Global Args Processing**: Skip over global arguments (determined by clap)
2. **Command Boundary Detection**: Identify known command names in remaining args
3. **Argument Collection**: Collect arguments between command boundaries
4. **Segment Creation**: Create `CommandSegment` for each detected command

```rust
pub fn segment_commands_only(
    &self,
    args: &[String],
    global_args: &[String],
) -> Result<Vec<CommandSegment>> {
    // Skip global args, process remaining for command segments
    let remaining_args = &args[global_args.len()..];

    for arg in remaining_args {
        if self.is_known_command(arg) {
            // Start new command segment
        } else {
            // Add to current command's arguments
        }
    }
}
```

## Plugin Activation

### Function Matching Process

The plugin manager matches command segments to plugin functions:

```rust
// For each command segment
for segment in command_segments {
    // Check all plugin functions
    for function in plugin_functions {
        // Match primary name or aliases
        if function.name == segment.command_name ||
           function.aliases.contains(&segment.command_name) {
            // Activate plugin with this function
            active_plugins.push(ActivePluginInfo {
                plugin_name: plugin_name.clone(),
                function_name: function.name.clone(),  // Always primary name
                args: segment.args.clone(),           // Command arguments
            });
        }
    }
}
```

### ActivePluginInfo Structure

When plugins are matched and activated, they're recorded as:

```rust
pub struct ActivePluginInfo {
    pub plugin_name: String,   // Name of the matched plugin
    pub function_name: String, // Primary function name (canonical)
    pub args: Vec<String>,     // Arguments from command segment
}
```

**Key Behaviours**:
- `function_name` always contains primary function name, never aliases
- `args` preserves exact command arguments from the segment
- Multiple plugins can be activated from a single CLI invocation

### Activation Examples

```bash
# Single plugin activation
repostats dump --format json
# → ActivePluginInfo { plugin_name: "dump", function_name: "dump", args: ["--format", "json"] }

# Multiple plugin activation
repostats analyse --since 1week export --type csv
# → ActivePluginInfo { plugin_name: "statistics", function_name: "analyse", args: ["--since", "1week"] }
# → ActivePluginInfo { plugin_name: "export", function_name: "export", args: ["--type", "csv"] }

# Using aliases
repostats analyze --detailed    # "analyze" is alias for "analyse"
# → ActivePluginInfo { plugin_name: "statistics", function_name: "analyse", args: ["--detailed"] }
```

## Argument Parsing Systems

### Two-Tier Architecture

Plugins can choose between two argument parsing approaches:

1. **Default Simple Parser**: Basic key-value and flag parsing (`PluginSettings`)
2. **Advanced clap Parser**: Full-featured CLI parsing with validation (`PluginArgParser`)

### Default Simple Parser (PluginSettings)

#### Basic Usage

```rust
impl Plugin for MyPlugin {
    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig
    ) -> PluginResult<()> {
        let settings = PluginSettings::from_cli_args("my_function".to_string(), args.to_vec())?;

        // Access parsed arguments
        if let Some(format) = settings.get_arg("format") {
            println!("Format: {}", format);
        }

        if settings.has_flag("verbose") {
            println!("Verbose mode enabled");
        }

        Ok(())
    }
}
```

#### Supported Argument Formats

```bash
# Key-value pairs (--key=value)
--format=json --output=/tmp/data --limit=100

# Flags (--flag or -f)
--verbose --debug -q -v

# Help (automatic)
--help, -h    # Generates help text and exits

# Mixed usage
repostats dump --format=json --verbose -q --limit=50
```

#### Argument Access

```rust
// Create settings from CLI args
let settings = PluginSettings::from_cli_args(function_name, args)?;

// Access key-value arguments
let format = settings.get_arg("format").unwrap_or(&"text".to_string());
let limit = settings.get_arg("limit")
    .and_then(|s| s.parse::<usize>().ok())
    .unwrap_or(10);

// Check flags
let verbose = settings.has_flag("verbose");
let debug = settings.has_flag("debug") || settings.has_flag("d");

// Raw access
let all_args = &settings.args;
let parsed_args = &settings.parsed_args;  // HashMap<String, String>
let flags = &settings.flags;              // HashSet<String>
```

#### Help Text Generation

The default parser automatically generates help text:

```rust
// Custom help text
impl PluginSettings {
    pub fn generate_help_text(&self) -> String {
        let function_name = self.function.as_deref().unwrap_or("plugin");
        format!(
            "Usage: {} [OPTIONS]\n\n\
             OPTIONS:\n  \
             -h, --help     Show this help message\n\n\
             Default plugin arguments parser. Individual plugins may extend this.",
            function_name
        )
    }
}
```

### Advanced clap Parser (PluginArgParser)

#### Setup and Usage

```rust
use crate::plugin::args::{PluginArgParser, PluginConfig, create_format_args};

impl Plugin for AdvancedPlugin {
    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig
    ) -> PluginResult<()> {
        // Create parser with plugin metadata
        let parser = PluginArgParser::new(
            "advanced",
            "Advanced plugin with sophisticated CLI",
            "1.0.0"
        )
        // Add standard format arguments
        .args(create_format_args())
        // Add custom arguments
        .arg(Arg::new("limit")
            .long("limit")
            .value_name("NUMBER")
            .help("Maximum number of items to process")
            .value_parser(clap::value_parser!(u32))
        )
        .arg(Arg::new("since")
            .long("since")
            .value_name("TIMESPEC")
            .help("Process items since this time")
            .required(false)
        );

        // Parse arguments
        let matches = parser.parse(args)?;

        // Extract values with type safety
        let limit = matches.get_one::<u32>("limit").unwrap_or(&100);
        let since = matches.get_one::<String>("since");
        let format = determine_format(&matches, config);

        println!("Limit: {}, Format: {}", limit, format);
        if let Some(since_value) = since {
            println!("Since: {}", since_value);
        }

        Ok(())
    }
}
```

#### Standard Format Arguments

The system provides pre-built format arguments for consistency:

```rust
pub fn create_format_args() -> Vec<Arg> {
    vec![
        Arg::new("json")
            .long("json")
            .action(clap::ArgAction::SetTrue)
            .help("Output in JSON format")
            .conflicts_with_all(&["text", "compact"]),
        Arg::new("text")
            .long("text")
            .action(clap::ArgAction::SetTrue)
            .help("Output in human-readable text format (default)")
            .conflicts_with_all(&["json", "compact"]),
        Arg::new("compact")
            .long("compact")
            .action(clap::ArgAction::SetTrue)
            .help("Output in compact single-line format")
            .conflicts_with_all(&["json", "text"]),
    ]
}
```

#### Format Detection with Configuration

```rust
pub fn determine_format(matches: &ArgMatches, config: &PluginConfig) -> OutputFormat {
    // CLI flags take precedence
    if matches.get_flag("json") { return OutputFormat::Json; }
    if matches.get_flag("compact") { return OutputFormat::Compact; }
    if matches.get_flag("text") { return OutputFormat::Text; }

    // Fallback to TOML configuration
    match config.get_string("default_format", "text").to_lowercase().as_str() {
        "json" => OutputFormat::Json,
        "compact" => OutputFormat::Compact,
        _ => OutputFormat::Text,
    }
}
```

#### Error Handling

The clap parser provides sophisticated error handling:

```rust
pub fn parse(&self, args: &[String]) -> PluginResult<ArgMatches> {
    match self.command.clone().try_get_matches_from(full_args) {
        Ok(matches) => Ok(matches),
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp |
            clap::error::ErrorKind::DisplayVersion => {
                // Print help/version and exit
                print!("{}", e);
                std::process::exit(0);
            }
            _ => {
                // Clean up error message for user
                let clean_msg = error_msg
                    .strip_prefix("error: ")
                    .unwrap_or(&error_msg)
                    .replace("unexpected argument", "Unknown argument")
                    .replace(" found", "");
                Err(PluginError::Generic { message: clean_msg })
            }
        }
    }
}
```

## Configuration Integration

### PluginConfig Structure

Plugins receive configuration context during argument parsing:

```rust
pub struct PluginConfig {
    pub use_colors: bool,                            // Global color setting
    pub toml_config: HashMap<String, toml::Value>,   // Plugin-specific TOML config
}
```

### Configuration Access

```rust
impl PluginConfig {
    // String values with defaults
    pub fn get_string(&self, key: &str, default: &str) -> String {
        if let Some(toml::Value::String(s)) = self.toml_config.get(key) {
            s.clone()
        } else {
            default.to_string()
        }
    }

    // Boolean values with defaults
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        if let Some(toml::Value::Boolean(b)) = self.toml_config.get(key) {
            *b
        } else {
            default
        }
    }
}
```

### TOML Integration Example

```toml
# Configuration file: repostats.toml
[dump]
default_format = "json"
show_headers = true
max_entries = 1000

[export]
default_format = "csv"
include_metadata = false
```

```rust
// Plugin receives this configuration
async fn parse_plugin_arguments(&mut self, args: &[String], config: &PluginConfig) -> PluginResult<()> {
    let default_format = config.get_string("default_format", "text");
    let show_headers = config.get_bool("show_headers", false);
    let max_entries = config.get_string("max_entries", "100")
        .parse::<usize>()
        .unwrap_or(100);

    // CLI arguments can override configuration
    // ...
}
```

## Complete Integration Example

### Plugin Implementation

```rust
use crate::plugin::traits::Plugin;
use crate::plugin::args::{PluginArgParser, PluginConfig, create_format_args, determine_format};

pub struct ExamplePlugin {
    format: OutputFormat,
    limit: u32,
    verbose: bool,
}

impl Plugin for ExamplePlugin {
    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig
    ) -> PluginResult<()> {
        let parser = PluginArgParser::new(
            "example",
            "Example plugin demonstrating CLI integration",
            "1.0.0"
        )
        .args(create_format_args())
        .arg(Arg::new("limit")
            .long("limit")
            .short('l')
            .value_name("N")
            .help("Limit output to N items")
            .value_parser(clap::value_parser!(u32))
        )
        .arg(Arg::new("verbose")
            .long("verbose")
            .short('v')
            .action(clap::ArgAction::SetTrue)
            .help("Enable verbose output")
        );

        let matches = parser.parse(args)?;

        // Extract and store configuration
        self.format = determine_format(&matches, config);
        self.limit = matches.get_one::<u32>("limit")
            .copied()
            .unwrap_or_else(|| {
                config.get_string("default_limit", "50")
                    .parse()
                    .unwrap_or(50)
            });
        self.verbose = matches.get_flag("verbose") || config.get_bool("verbose", false);

        if self.verbose {
            println!("Plugin configured: format={}, limit={}", self.format, self.limit);
        }

        Ok(())
    }

    // ... other plugin methods
}
```

### CLI Usage Examples

```bash
# Using defaults from configuration
repostats example

# Override format
repostats example --json

# Complex usage with multiple arguments
repostats example --limit 25 --verbose --compact

# Using short flags
repostats example -l 10 -v --json

# Multiple plugins in one command
repostats example --limit 5 export --format csv

# Global args + plugin args
repostats --config custom.toml --log-level debug example --verbose --limit 100
```

### Error Handling Examples

```bash
# Invalid argument
$ repostats example --invalid-flag
Error: Unknown argument '--invalid-flag'

# Help request
$ repostats example --help
example 1.0.0
Example plugin demonstrating CLI integration

USAGE:
    example [OPTIONS]

OPTIONS:
    -h, --help               Show help information
    -v, --verbose            Enable verbose output
    -l, --limit <N>          Limit output to N items
        --json               Output in JSON format
        --text               Output in human-readable text format (default)
        --compact            Output in compact single-line format
```

## Best Practices

### Argument Design

1. **Consistent Naming**: Use consistent argument names across plugins
   - ✅ `--format`, `--limit`, `--verbose`
   - ❌ `--fmt`, `--max`, `--verb`

2. **Short Flags**: Provide short versions for common arguments
   - ✅ `--verbose` / `-v`, `--limit` / `-l`
   - ❌ Only long forms for frequently used options

3. **Default Values**: Always provide sensible defaults
   ```rust
   let limit = matches.get_one::<u32>("limit").unwrap_or(&100);
   ```

### Parser Selection

**Use Simple Parser (`PluginSettings`) when**:
- Simple key-value and flag parsing is sufficient
- Minimal validation requirements
- Quick prototyping or basic plugins

**Use clap Parser (`PluginArgParser`) when**:
- Complex argument validation needed
- Type safety required
- Professional help text generation
- Argument conflicts or dependencies

### Configuration Integration

1. **Layer Configuration**: CLI args override TOML config override defaults
2. **Provide Fallbacks**: Always have working defaults
3. **Document Config**: Include configuration options in help text

### Error Handling

1. **Clear Messages**: Provide helpful error messages
2. **Help Integration**: Make help easily accessible
3. **Graceful Degradation**: Handle partial failures appropriately

## Common Patterns

### Boolean Flag with Configuration

```rust
let enabled = matches.get_flag("enable") ||
              config.get_bool("default_enabled", false);
```

### Numeric Value with Validation

```rust
let limit = matches.get_one::<u32>("limit")
    .copied()
    .or_else(|| config.get_string("default_limit", "100").parse().ok())
    .unwrap_or(100)
    .min(1000)  // Clamp maximum
    .max(1);    // Clamp minimum
```

### Optional String with Fallback Chain

```rust
let output_path = matches.get_one::<String>("output")
    .cloned()
    .or_else(|| config.get_string("default_output_path", "").into())
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "/tmp/output".to_string());
```

## See Also

- [Plugin Functions Documentation](plugin_functions.md)
- [PluginInfo Documentation](plugin_info.md)
- [System Architecture Documentation](../system_architecture/plugin_integration.md)
- [Plugin Author Guide](plugin_author_guide.md)
