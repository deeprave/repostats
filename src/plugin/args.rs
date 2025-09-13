//! Plugin Argument Parsing
//!
//! Provides proper clap-based argument parsing for plugins with color support,
//! TOML configuration integration, and extensible command building.

use crate::plugin::error::{PluginError, PluginResult};
use clap::{Arg, ArgAction, ArgMatches, Command};
use std::collections::HashMap;

/// Configuration context passed to plugins during initialization
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Forced color setting: Some(true)=force on, Some(false)=force off, None=auto (TTY based)
    pub use_colors: Option<bool>,
    /// Plugin-specific TOML configuration
    pub toml_config: HashMap<String, toml::Value>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            use_colors: None,
            toml_config: HashMap::new(),
        }
    }
}

impl PluginConfig {
    /// Create a PluginConfig from a TOML table
    pub fn from_toml(use_colors: Option<bool>, toml_table: &toml::value::Table) -> Self {
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

    /// Set a string configuration value (for testing)
    #[cfg(test)]
    pub fn set_string(&mut self, key: &str, value: &str) {
        self.toml_config
            .insert(key.to_string(), toml::Value::String(value.to_string()));
    }
}

/// Base argument parser for plugins
pub struct PluginArgParser {
    command: Command,
    plugin_name: String,
}

impl PluginArgParser {
    /// Create a new plugin argument parser
    ///
    /// Caller supplies whether colors should be used (from global config/environment)
    pub fn new(
        plugin_name: &str,
        description: &str,
        version: &str,
        use_colors: Option<bool>,
    ) -> Self {
        let colors =
            use_colors.unwrap_or_else(|| std::io::IsTerminal::is_terminal(&std::io::stdout()));

        let command = Command::new(plugin_name.to_string())
            .about(description.to_string())
            .version(version.to_string())
            .disable_version_flag(true)
            .disable_help_flag(true)
            .color(Self::color_choice(use_colors))
            .styles(Self::get_help_styles(colors))
            .arg(
                clap::Arg::new("version")
                    .short('v')
                    .long("version")
                    .action(ArgAction::Version)
                    .help("Print version"),
            )
            .arg(
                clap::Arg::new("help")
                    .short('h')
                    .long("help")
                    .action(ArgAction::Help)
                    .help("Print help"),
            );

        Self {
            command,
            plugin_name: plugin_name.to_string(),
        }
    }

    fn get_help_styles(colors_enabled: bool) -> clap::builder::Styles {
        crate::core::styles::palette_to_clap(colors_enabled)
    }

    fn color_choice(color_setting: Option<bool>) -> clap::ColorChoice {
        match color_setting {
            Some(true) => clap::ColorChoice::Always,
            Some(false) => clap::ColorChoice::Never,
            None => clap::ColorChoice::Auto,
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
        match self.command.clone().try_get_matches_from(args) {
            Ok(matches) => Ok(matches),
            Err(e) => match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    // Print help or version directly and exit successfully
                    // This is what clap normally does for the main program
                    // Use print() method to preserve color formatting
                    let _ = e.print();
                    std::process::exit(0);
                }
                _ => {
                    // Preserve clap's full output (which includes Usage:) so the user sees context.
                    let full = e.to_string();
                    let mut core_line = full.lines().next().unwrap_or("").trim();
                    if let Some(stripped) = core_line.strip_prefix("error: ") {
                        core_line = stripped.trim();
                    }
                    let simplified = core_line.to_string();
                    // Do NOT append clap help/usage block per request; return single-line message.
                    Err(PluginError::Generic {
                        message: simplified,
                    })
                }
            },
        }
    }
}
