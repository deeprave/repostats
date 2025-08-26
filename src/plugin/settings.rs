//! Plugin Settings and CLI Argument Parsing
//!
//! Provides plugin-specific configuration management with CLI argument parsing support.
//! Plugins can extend the default parser for their specific needs.

use crate::plugin::error::{PluginError, PluginResult};

/// Plugin settings with CLI argument parsing support
#[derive(Debug, Clone)]
pub struct PluginSettings {
    /// Plugin function invoked
    pub function: Option<String>,

    /// Command-line arguments passed to the plugin
    pub args: Vec<String>,

    /// Whether plugin is active (activated via CLI)
    pub active: bool,

    /// Parsed key-value arguments
    pub parsed_args: std::collections::HashMap<String, String>,

    /// Boolean flags
    pub flags: std::collections::HashSet<String>,
}

impl PluginSettings {
    /// Create new plugin settings
    pub fn new() -> Self {
        Self {
            function: None,
            args: Vec::new(),
            active: false,
            parsed_args: std::collections::HashMap::new(),
            flags: std::collections::HashSet::new(),
        }
    }

    /// Create settings from CLI arguments (activates plugin)
    pub fn from_cli_args(function: String, args: Vec<String>) -> PluginResult<Self> {
        let mut settings = Self::new();
        settings.function = Some(function);
        settings.args = args.clone();
        settings.active = true;

        // Parse arguments using default parser
        settings.parse_arguments(&args)?;

        Ok(settings)
    }

    /// Default argument parser implementation that plugins can extend
    pub fn parse_arguments(&mut self, args: &[String]) -> PluginResult<()> {
        for arg in args {
            if arg == "--help" || arg == "-h" {
                return Err(PluginError::Generic {
                    message: self.generate_help_text(),
                });
            } else if let Some(stripped) = arg.strip_prefix("--") {
                // Handle --key=value or --key value formats
                if let Some(eq_pos) = stripped.find('=') {
                    let key = stripped[..eq_pos].to_string();
                    let value = stripped[eq_pos + 1..].to_string();
                    self.parsed_args.insert(key, value);
                } else {
                    // It's a flag
                    let key = stripped.to_string();
                    self.flags.insert(key);
                }
            } else if arg.starts_with('-') && arg.len() > 1 {
                // Handle short flags like -v
                let key = arg[1..].to_string();
                self.flags.insert(key);
            } else if !arg.is_empty() && !arg.starts_with('-') {
                // Invalid argument format
                return Err(PluginError::Generic {
                    message: format!(
                        "Invalid argument format: '{}'. Use --key=value or --flag format.",
                        arg
                    ),
                });
            }
        }

        Ok(())
    }

    /// Generate help text for the plugin (can be overridden)
    pub fn generate_help_text(&self) -> String {
        let function_name = self.function.as_deref().unwrap_or("plugin");

        format!(
            "Usage: {} [OPTIONS]\n\nOPTIONS:\n  -h, --help     Show this help message\n\nDefault plugin arguments parser. Individual plugins may extend this with additional options.",
            function_name
        )
    }

    /// Check if plugin is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get parsed argument value
    pub fn get_arg(&self, key: &str) -> Option<&String> {
        self.parsed_args.get(key)
    }

    /// Check if flag is set
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }

    /// Validate arguments (can be extended by plugins)
    pub fn validate(&self) -> PluginResult<()> {
        // Default validation - no specific requirements
        Ok(())
    }
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_settings_creation() {
        let settings = PluginSettings::new();

        assert_eq!(settings.function, None);
        assert!(settings.args.is_empty());
        assert!(!settings.active);
        assert!(settings.parsed_args.is_empty());
        assert!(settings.flags.is_empty());
    }

    #[test]
    fn test_plugin_settings_default() {
        let settings = PluginSettings::default();

        assert_eq!(settings.function, None);
        assert!(settings.args.is_empty());
        assert!(!settings.active);
    }

    #[test]
    fn test_plugin_settings_from_cli_args() {
        let args = vec![
            "--output=json".to_string(),
            "--verbose".to_string(),
            "-q".to_string(),
        ];

        let settings = PluginSettings::from_cli_args("dump".to_string(), args.clone()).unwrap();

        assert_eq!(settings.function, Some("dump".to_string()));
        assert_eq!(settings.args, args);
        assert!(settings.active);
        assert!(settings.is_active());

        // Check parsed arguments
        assert_eq!(settings.get_arg("output"), Some(&"json".to_string()));
        assert!(settings.has_flag("verbose"));
        assert!(settings.has_flag("q"));
    }

    #[test]
    fn test_argument_parsing_key_value_pairs() {
        let mut settings = PluginSettings::new();

        let args = vec![
            "--format=csv".to_string(),
            "--count=100".to_string(),
            "--path=/tmp/output".to_string(),
        ];

        settings.parse_arguments(&args).unwrap();

        assert_eq!(settings.get_arg("format"), Some(&"csv".to_string()));
        assert_eq!(settings.get_arg("count"), Some(&"100".to_string()));
        assert_eq!(settings.get_arg("path"), Some(&"/tmp/output".to_string()));
    }

    #[test]
    fn test_argument_parsing_flags() {
        let mut settings = PluginSettings::new();

        let args = vec![
            "--verbose".to_string(),
            "--debug".to_string(),
            "-q".to_string(),
            "-v".to_string(),
        ];

        settings.parse_arguments(&args).unwrap();

        assert!(settings.has_flag("verbose"));
        assert!(settings.has_flag("debug"));
        assert!(settings.has_flag("q"));
        assert!(settings.has_flag("v"));
    }

    #[test]
    fn test_argument_parsing_mixed() {
        let mut settings = PluginSettings::new();

        let args = vec![
            "--output=json".to_string(),
            "--verbose".to_string(),
            "--limit=50".to_string(),
            "-q".to_string(),
        ];

        settings.parse_arguments(&args).unwrap();

        // Key-value pairs
        assert_eq!(settings.get_arg("output"), Some(&"json".to_string()));
        assert_eq!(settings.get_arg("limit"), Some(&"50".to_string()));

        // Flags
        assert!(settings.has_flag("verbose"));
        assert!(settings.has_flag("q"));

        // Not present
        assert_eq!(settings.get_arg("nonexistent"), None);
        assert!(!settings.has_flag("nonexistent"));
    }

    #[test]
    fn test_help_flag_generates_error_with_help_text() {
        let mut settings = PluginSettings::new();
        settings.function = Some("test-plugin".to_string());

        let args = vec!["--help".to_string()];
        let result = settings.parse_arguments(&args);

        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::Generic { message } => {
                assert!(message.contains("Usage: test-plugin [OPTIONS]"));
                assert!(message.contains("--help"));
                assert!(message.contains("Show this help message"));
            }
            _ => panic!("Expected Generic error with help text"),
        }
    }

    #[test]
    fn test_short_help_flag() {
        let mut settings = PluginSettings::new();
        settings.function = Some("test".to_string());

        let args = vec!["-h".to_string()];
        let result = settings.parse_arguments(&args);

        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::Generic { message } => {
                assert!(message.contains("Usage: test [OPTIONS]"));
            }
            _ => panic!("Expected Generic error with help text"),
        }
    }

    #[test]
    fn test_invalid_argument_format() {
        let mut settings = PluginSettings::new();

        let args = vec!["invalid_arg_without_dashes".to_string()];

        let result = settings.parse_arguments(&args);
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::Generic { message } => {
                assert!(message.contains("Invalid argument format"));
                assert!(message.contains("invalid_arg_without_dashes"));
            }
            _ => panic!("Expected Generic error for invalid format"),
        }
    }

    #[test]
    fn test_empty_args_handling() {
        let mut settings = PluginSettings::new();

        let args = vec![
            "".to_string(), // Empty string should be ignored
            "--valid".to_string(),
        ];

        settings.parse_arguments(&args).unwrap();
        assert!(settings.has_flag("valid"));
    }

    #[test]
    fn test_validation_default() {
        let settings = PluginSettings::new();

        // Default validation should always pass
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_help_text_generation() {
        let mut settings = PluginSettings::new();
        settings.function = Some("custom-plugin".to_string());

        let help = settings.generate_help_text();

        assert!(help.contains("Usage: custom-plugin [OPTIONS]"));
        assert!(help.contains("-h, --help"));
        assert!(help.contains("Show this help message"));
    }

    #[test]
    fn test_help_text_generation_without_function() {
        let settings = PluginSettings::new();

        let help = settings.generate_help_text();

        assert!(help.contains("Usage: plugin [OPTIONS]"));
    }

    #[test]
    fn test_plugin_activation() {
        let inactive_settings = PluginSettings::new();
        assert!(!inactive_settings.is_active());

        let active_settings =
            PluginSettings::from_cli_args("dump".to_string(), vec!["--verbose".to_string()])
                .unwrap();
        assert!(active_settings.is_active());
    }
}
