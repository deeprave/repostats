//! Plugin Argument Parsing
//!
//! Provides proper clap-based argument parsing for plugins with color support,
//! TOML configuration integration, and extensible command building.

use crate::plugin::error::{PluginError, PluginResult};
use clap::{Arg, ArgMatches, Command};
use std::collections::HashMap;

/// Configuration context passed to plugins during initialization
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Whether colors should be used (already determined at global level)
    pub use_colors: bool,
    /// Plugin-specific TOML configuration
    pub toml_config: HashMap<String, toml::Value>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            use_colors: false,
            toml_config: HashMap::new(),
        }
    }
}

impl PluginConfig {
    /// Create a PluginConfig from a TOML table
    pub fn from_toml(use_colors: bool, toml_table: &toml::value::Table) -> Self {
        let mut config = HashMap::new();
        for (key, value) in toml_table.iter() {
            config.insert(key.clone(), value.clone());
        }
        Self {
            use_colors,
            toml_config: config,
        }
    }

    /// Get a string configuration value with default
    pub fn get_string(&self, key: &str, default: &str) -> String {
        if let Some(toml::Value::String(s)) = self.toml_config.get(key) {
            s.clone()
        } else {
            default.to_string()
        }
    }

    /// Get a boolean configuration value with default
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        if let Some(toml::Value::Boolean(b)) = self.toml_config.get(key) {
            *b
        } else {
            default
        }
    }
}

/// Base argument parser for plugins
pub struct PluginArgParser {
    command: Command,
    plugin_name: String,
}

impl PluginArgParser {
    /// Create a new plugin argument parser
    pub fn new(plugin_name: &str, description: &str, version: &str) -> Self {
        let command = Command::new(plugin_name.to_string())
            .about(description.to_string())
            .version(version.to_string())
            .disable_help_flag(true)
            .arg(
                Arg::new("help")
                    .long("help")
                    .short('h')
                    .action(clap::ArgAction::Help)
                    .help("Show help information"),
            );
        // Note: clap automatically adds --version when .version() is set

        Self {
            command,
            plugin_name: plugin_name.to_string(),
        }
    }

    /// Add a custom argument to the command
    pub fn arg(mut self, arg: Arg) -> Self {
        self.command = self.command.arg(arg);
        self
    }

    /// Add multiple arguments to the command
    pub fn args(mut self, args: impl IntoIterator<Item = Arg>) -> Self {
        self.command = self.command.args(args);
        self
    }

    /// Parse arguments and return matches
    pub fn parse(&self, args: &[String]) -> PluginResult<ArgMatches> {
        // clap expects the first argument to be the program name
        let mut full_args = vec![self.plugin_name.as_str()];
        full_args.extend(args.iter().map(|s| s.as_str()));

        match self.command.clone().try_get_matches_from(full_args) {
            Ok(matches) => Ok(matches),
            Err(e) => match e.kind() {
                clap::error::ErrorKind::DisplayHelp => Err(PluginError::Generic {
                    message: e.to_string(),
                }),
                clap::error::ErrorKind::DisplayVersion => Err(PluginError::Generic {
                    message: format!("{} version information", self.plugin_name),
                }),
                _ => Err(PluginError::Generic {
                    message: format!("Argument parsing error: {}", e),
                }),
            },
        }
    }
}

/// Create standard format arguments for output plugins
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

/// Determine output format from parsed arguments and config
pub fn determine_format(matches: &ArgMatches, config: &PluginConfig) -> OutputFormat {
    if matches.get_flag("json") {
        return OutputFormat::Json;
    }
    if matches.get_flag("compact") {
        return OutputFormat::Compact;
    }
    if matches.get_flag("text") {
        return OutputFormat::Text;
    }

    // Check TOML configuration for default format
    match config
        .get_string("default_format", "text")
        .to_lowercase()
        .as_str()
    {
        "json" => OutputFormat::Json,
        "compact" => OutputFormat::Compact,
        _ => OutputFormat::Text,
    }
}

/// Standard output formats for plugins
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
    Compact,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Compact => write!(f, "compact"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_config_default() {
        let config = PluginConfig::default();
        assert!(!config.use_colors);
        assert!(config.toml_config.is_empty());
    }

    #[test]
    fn test_plugin_config_get_methods() {
        let mut config = PluginConfig::default();
        config.toml_config.insert(
            "test_key".to_string(),
            toml::Value::String("test_value".to_string()),
        );
        config
            .toml_config
            .insert("test_bool".to_string(), toml::Value::Boolean(true));

        assert_eq!(config.get_string("test_key", "default"), "test_value");
        assert_eq!(config.get_string("missing_key", "default"), "default");
        assert!(config.get_bool("test_bool", false));
        assert!(!config.get_bool("missing_bool", false));
    }

    #[test]
    fn test_plugin_arg_parser() {
        let parser =
            PluginArgParser::new("test", "Test plugin", "1.0.0").args(create_format_args());

        let matches = parser.parse(&["--json".to_string()]).unwrap();
        assert!(matches.get_flag("json"));
    }

    #[test]
    fn test_determine_format() {
        let parser =
            PluginArgParser::new("test", "Test plugin", "1.0.0").args(create_format_args());
        let config = PluginConfig::default();

        // Test JSON format
        let matches = parser.parse(&["--json".to_string()]).unwrap();
        assert!(matches.get_flag("json"), "JSON flag should be true");
        assert!(!matches.get_flag("text"), "Text flag should be false");
        assert!(!matches.get_flag("compact"), "Compact flag should be false");
        assert_eq!(determine_format(&matches, &config), OutputFormat::Json);

        // Test compact format
        let matches = parser.parse(&["--compact".to_string()]).unwrap();
        assert!(matches.get_flag("compact"), "Compact flag should be true");
        assert!(!matches.get_flag("json"), "JSON flag should be false");
        assert!(!matches.get_flag("text"), "Text flag should be false");
        assert_eq!(determine_format(&matches, &config), OutputFormat::Compact);

        // Test default (no flags)
        let matches = parser.parse(&[]).unwrap();
        assert!(
            !matches.get_flag("json"),
            "JSON flag should be false by default"
        );
        assert!(
            !matches.get_flag("text"),
            "Text flag should be false by default"
        );
        assert!(
            !matches.get_flag("compact"),
            "Compact flag should be false by default"
        );
        assert_eq!(determine_format(&matches, &config), OutputFormat::Text);
    }
}
