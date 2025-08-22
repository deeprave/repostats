//! Minimal initial argument parsing for configuration discovery
//!
//! This module provides a first-stage parser that uses clap to extract only
//! the configuration-related arguments needed before plugin discovery.
//! It uses clap's ability to ignore unknown arguments, ensuring we handle
//! all CLI edge cases (short flags, equals syntax, etc.) correctly.

use clap::{Parser, ArgAction};
use std::path::PathBuf;

/// Minimal clap parser for configuration discovery only
///
/// This parser uses clap's derive API but with settings that allow
/// it to ignore unknown arguments gracefully. Only configuration-related
/// arguments are captured here.
#[derive(Parser, Debug, Clone)]
#[command(name = "repostats")]
#[command(disable_help_flag = true)]  // We'll handle help ourselves
#[command(disable_version_flag = true)]  // We'll handle version ourselves
#[command(ignore_errors = true)]  // Ignore unknown arguments/subcommands
pub struct InitialArgs {
    /// Configuration file path
    #[arg(long = "config-file", value_name = "FILE")]
    pub config_file: Option<PathBuf>,

    /// Plugin directory override
    #[arg(long = "plugin-dir", value_name = "DIR")]
    pub plugin_dir: Option<String>,

    /// Plugin exclusion list
    #[arg(long = "plugin-exclude", value_name = "LIST")]
    pub plugin_exclude: Option<String>,

    /// Verbose output (can be used multiple times for more verbosity)
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    pub verbose: u8,

    /// Quiet mode (can be used multiple times for less verbosity)
    #[arg(short = 'q', long = "quiet", action = ArgAction::Count)]
    pub quiet: u8,

    /// Force colored output (overrides TTY detection and NO_COLOR)
    #[arg(long = "color", action = ArgAction::SetTrue)]
    pub color: bool,

    /// Disable colored output (overrides configuration and NO_COLOR)
    #[arg(long = "no-color", action = ArgAction::SetTrue)]
    pub no_color: bool,

    /// Log output format
    #[arg(long = "log-format", value_name = "FORMAT", value_parser = ["text", "json"])]
    pub log_format: Option<String>,

    /// Log level
    #[arg(long = "log-level", value_name = "LEVEL", value_parser = ["trace", "debug", "info", "warn", "error", "off"])]
    pub log_level: Option<String>,

    /// Log file path (use 'none' to disable file logging)
    #[arg(long = "log-file", value_name = "FILE")]
    pub log_file: Option<PathBuf>,
}

impl InitialArgs {
    /// Get the command name from the clap metadata
    pub fn command_name() -> String {
        use clap::CommandFactory;
        Self::command().get_name().to_string()
    }

    /// Parse minimal arguments from command line using clap with proper error handling
    ///
    /// This builds a clap parser that only includes configuration-related arguments
    /// and uses clap's proper parsing with error handling for unknown arguments.
    pub fn parse_from_env() -> Self {
        use std::env;
        let args: Vec<String> = env::args().collect();
        Self::parse_from_args(&args)
    }

    /// Parse minimal arguments from a provided argument list using clap's try_parse_from
    pub fn parse_from_args(args: &[String]) -> Self {
        let cmd = clap::Command::new("repostats")  // Use hardcoded name to avoid lifetime issues
            .disable_help_flag(true)  // We'll handle help manually
            .disable_version_flag(true)  // We'll handle version manually
            .arg(clap::Arg::new("config-file")
                .long("config-file")
                .value_name("FILE")
                .help("Configuration file path"))
            .arg(clap::Arg::new("plugin-dir")
                .long("plugin-dir")
                .value_name("DIR")
                .help("Plugin directory override"))
            .arg(clap::Arg::new("plugin-exclude")
                .long("plugin-exclude")
                .value_name("LIST")
                .help("Plugin exclusion list"))
            .arg(clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(clap::ArgAction::Count)
                .help("Verbose output (can be used multiple times for more verbosity)"))
            .arg(clap::Arg::new("quiet")
                .short('q')
                .long("quiet")
                .action(clap::ArgAction::Count)
                .help("Quiet mode (can be used multiple times for less verbosity)"))
            .arg(clap::Arg::new("color")
                .long("color")
                .action(clap::ArgAction::SetTrue)
                .help("Force colored output"))
            .arg(clap::Arg::new("no-color")
                .long("no-color")
                .action(clap::ArgAction::SetTrue)
                .help("Disable colored output"))
            .arg(clap::Arg::new("log-format")
                .long("log-format")
                .value_name("FORMAT")
                .value_parser(["text", "json"])
                .help("Log output format (text, json)"))
            .arg(clap::Arg::new("log-level")
                .long("log-level")
                .value_name("LEVEL")
                .value_parser(["trace", "debug", "info", "warn", "error", "off"])
                .help("Log level (trace, debug, info, warn, error, off)"))
            .arg(clap::Arg::new("log-file")
                .long("log-file")
                .value_name("FILE")
                .help("Log file path (use 'none' to disable file logging)"))
            .allow_external_subcommands(true)
            .ignore_errors(true);

        match cmd.try_get_matches_from(args) {
            Ok(matches) => Self::from_matches(&matches),
            Err(_) => Self::create_minimal_fallback(),
        }
    }

    /// Create InitialArgs from clap ArgMatches
    fn from_matches(matches: &clap::ArgMatches) -> Self {
        Self {
            config_file: matches.get_one::<String>("config-file")
                .map(|s| std::path::PathBuf::from(s)),
            plugin_dir: matches.get_one::<String>("plugin-dir").cloned(),
            plugin_exclude: matches.get_one::<String>("plugin-exclude").cloned(),
            verbose: matches.get_count("verbose"),
            quiet: matches.get_count("quiet"),
            color: matches.get_flag("color"),
            no_color: matches.get_flag("no-color"),
            log_format: matches.get_one::<String>("log-format").cloned(),
            log_level: matches.get_one::<String>("log-level").cloned(),
            log_file: matches.get_one::<String>("log-file")
                .filter(|s| *s != "none")
                .map(|s| PathBuf::from(s)),
        }
    }

    /// Create a minimal fallback when initial parsing fails
    fn create_minimal_fallback() -> Self {
        Self {
            config_file: None,
            plugin_dir: None,
            plugin_exclude: None,
            verbose: 0,
            quiet: 0,
            color: false,
            no_color: false,
            log_format: None,
            log_level: None,
            log_file: None,
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config_file() {
        let args = vec![
            "repostats".to_string(),
            "--config-file".to_string(),
            "custom.toml".to_string(),
            "commits".to_string(),  // Unknown subcommand, but should be ignored
        ];

        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.config_file, Some(PathBuf::from("custom.toml")));
    }

    #[test]
    fn test_parse_plugin_dir() {
        let args = vec![
            "repostats".to_string(),
            "--plugin-dir".to_string(),
            "/custom/plugins".to_string(),
            "--plugin-exclude".to_string(),
            "unwanted".to_string(),
            "output".to_string(),  // Unknown subcommand, should be ignored
        ];

        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.plugin_dir, Some("/custom/plugins".to_string()));
        assert_eq!(initial.plugin_exclude, Some("unwanted".to_string()));
    }

    #[test]
    fn test_mixed_known_unknown_args() {
        let args = vec![
            "repostats".to_string(),
            "--config-file".to_string(),
            "test.toml".to_string(),
            "--verbose".to_string(),     // Unknown to initial parser
            "commits".to_string(),       // Unknown subcommand
            "--since".to_string(),       // Unknown to initial parser
            "1 week".to_string(),        // Unknown argument
        ];

        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.config_file, Some(PathBuf::from("test.toml")));
    }


    #[test]
    fn test_equals_syntax() {
        let args = vec![
            "repostats".to_string(),
            "--config-file=custom.toml".to_string(),
            "--plugin-dir=/plugins".to_string(),
            "commits".to_string(),
        ];

        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.config_file, Some(PathBuf::from("custom.toml")));
        assert_eq!(initial.plugin_dir, Some("/plugins".to_string()));
    }

    #[test]
    fn test_fallback_on_parse_failure() {
        let args = vec![
            "repostats".to_string(),
            "--config-file".to_string(),
            // Missing value for config-file
        ];

        // Should not panic, should return fallback
        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.config_file, None);
    }

    #[test]
    fn test_color_flags() {
        let args = vec![
            "repostats".to_string(),
            "--color".to_string(),
        ];

        let initial = InitialArgs::parse_from_args(&args);
        assert!(initial.color);
        assert!(!initial.no_color);

        let args = vec![
            "repostats".to_string(),
            "--no-color".to_string(),
        ];

        let initial = InitialArgs::parse_from_args(&args);
        assert!(!initial.color);
        assert!(initial.no_color);
    }

}