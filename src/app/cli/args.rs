//! Global arguments parsing for the full command line interface
//!
//! This module handles the complete parsing of global arguments after segmentation.
//! It uses standard clap parsing since we have clean global args at this stage.

use clap::Parser;
use std::path::PathBuf;

// Global arguments structure with all command-line options
//
// Used for both initial parsing (before command discovery) and final parsing
// (after command segmentation). Handles all global configuration flags.
#[derive(Parser, Debug, Clone)]
#[command(name = "repostats")]
#[command(about = "Repository statistics and analysis tool")]
#[command(version)]
pub struct Args {
    /// Repository path to analyse (defaults to current directory)
    #[arg(short = 'r', long = "repo", value_name = "PATH")]
    pub repository: Option<PathBuf>,

    /// Configuration file path
    #[arg(long = "config-file", value_name = "FILE")]
    pub config_file: Option<PathBuf>,

    /// Plugin directory override
    #[arg(long = "plugin-dir", value_name = "DIR")]
    pub plugin_dir: Option<String>,

    /// Plugin exclusion list
    #[arg(long = "plugin-exclude", value_name = "LIST")]
    pub plugin_exclude: Option<String>,

    /// Force colored output (overrides TTY detection and NO_COLOR)
    #[arg(long = "color")]
    pub color: bool,

    /// Disable colored output
    #[arg(long = "no-color", conflicts_with = "color")]
    pub no_color: bool,

    /// Log level
    #[arg(long = "log-level", value_name = "LEVEL", value_parser = ["trace", "debug", "info", "warn", "error", "off"])]
    pub log_level: Option<String>,

    /// Log file path (use 'none' to disable file logging)
    #[arg(
        long = "log-file",
        value_name = "FILE",
        help = "Log file path (use 'none' to disable file logging)"
    )]
    pub log_file: Option<PathBuf>,

    /// Log output format
    #[arg(long = "log-format", value_name = "FORMAT", value_parser = ["text", "ext", "json"])]
    pub log_format: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            repository: None,
            config_file: None,
            plugin_dir: None,
            plugin_exclude: None,
            color: false,
            no_color: false,
            log_level: None,
            log_file: None,
            log_format: Some("text".to_string()), // Default format
        }
    }
}

impl Args {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse initial global arguments using clap, stopping at first external subcommand
    ///
    /// Uses the same clap-based approach as parse_from_args but configured for initial parsing.
    /// Stops at the first external subcommand (assumed to be a command) and returns both
    /// the parsed arguments and the consumed argument list.
    pub fn parse_initial(command_name: &str, args: &[String]) -> (Self, Vec<String>) {
        use clap::FromArgMatches;
        use std::path::PathBuf;

        if args.is_empty() {
            return (Self::default(), Vec::new());
        }

        // Use the exact same clap setup as parse_from_args
        let cmd = clap::Command::new(command_name.to_string())
            .about("Repository statistics and analysis tool")
            .version(env!("CARGO_PKG_VERSION"))
            .allow_external_subcommands(true) // Allow unknown commands (stops at first one)
            .ignore_errors(false) // Handle errors properly
            .arg(
                clap::Arg::new("repository")
                    .short('r')
                    .long("repo")
                    .value_name("PATH")
                    .value_parser(clap::value_parser!(PathBuf))
                    .help("Repository path to analyze (defaults to current directory)"),
            )
            .arg(
                clap::Arg::new("config_file")
                    .long("config-file")
                    .value_name("FILE")
                    .value_parser(clap::value_parser!(PathBuf))
                    .help("Configuration file path"),
            )
            .arg(
                clap::Arg::new("plugin_dir")
                    .long("plugin-dir")
                    .value_name("DIR")
                    .help("Plugin directory override"),
            )
            .arg(
                clap::Arg::new("plugin_exclude")
                    .long("plugin-exclude")
                    .value_name("LIST")
                    .help("Plugin exclusion list"),
            )
            .arg(
                clap::Arg::new("color")
                    .long("color")
                    .action(clap::ArgAction::SetTrue)
                    .help("Force colored output (overrides TTY detection and NO_COLOR)"),
            )
            .arg(
                clap::Arg::new("no_color")
                    .long("no-color")
                    .conflicts_with("color")
                    .action(clap::ArgAction::SetTrue)
                    .help("Disable colored output"),
            )
            .arg(
                clap::Arg::new("log_level")
                    .long("log-level")
                    .value_name("LEVEL")
                    .value_parser(["trace", "debug", "info", "warn", "error", "off"])
                    .help("Log level"),
            )
            .arg(
                clap::Arg::new("log_file")
                    .long("log-file")
                    .value_name("FILE")
                    .value_parser(clap::value_parser!(PathBuf))
                    .help("Log file path (use 'none' to disable file logging)"),
            )
            .arg(
                clap::Arg::new("log_format")
                    .long("log-format")
                    .value_name("FORMAT")
                    .value_parser(["text", "ext", "json"])
                    .help("Log output format"),
            );

        match cmd.try_get_matches_from(args) {
            Ok(matches) => {
                // Parse using clap's standard mechanism
                let mut parsed_args = Self::from_arg_matches(&matches).unwrap_or_default();

                // Handle special "none" value for log_file (same as parse_from_args)
                if let Some(ref path) = parsed_args.log_file {
                    if path.to_string_lossy() == "none" {
                        parsed_args.log_file = None;
                    }
                }

                // Extract the consumed arguments
                let global_args = Self::extract_consumed_args(args, &matches);
                (parsed_args, global_args)
            }
            Err(e) => {
                // Let clap handle help/version/errors properly
                e.print().expect("Error writing to stderr");
                std::process::exit(e.exit_code());
            }
        }
    }

    /// Extract the arguments that clap actually consumed during parsing
    fn extract_consumed_args(original_args: &[String], matches: &clap::ArgMatches) -> Vec<String> {
        let mut consumed_args = vec![original_args[0].clone()]; // Always include program name

        // Find where clap stopped parsing (first external subcommand)
        if let Some((external_subcommand, _)) = matches.subcommand() {
            // Find the position of the external subcommand in original args
            if let Some(pos) = original_args[1..]
                .iter()
                .position(|arg| arg == external_subcommand)
            {
                // Include arguments up to (but not including) the external subcommand
                consumed_args.extend(original_args[1..pos + 1].iter().cloned());
            } else {
                // If we can't find the subcommand, include all arguments except the last one
                // (this handles edge cases where clap's parsing differs from our expectation)
                if original_args.len() > 1 {
                    consumed_args.extend(original_args[1..original_args.len() - 1].iter().cloned());
                }
            }
        } else {
            // No subcommand found, clap consumed all arguments (help/version case)
            consumed_args.extend(original_args[1..].iter().cloned());
        }

        consumed_args
    }

    fn color_choice(color: bool, no_color: bool) -> clap::ColorChoice {
        if color {
            clap::ColorChoice::Always
        } else if no_color {
            clap::ColorChoice::Never
        } else {
            clap::ColorChoice::Auto
        }
    }
    /// Parse global arguments from a provided argument list
    ///
    pub fn parse_from_args(
        margs: &mut Self,
        command_name: &str,
        args: &[String],
        color: bool,
        no_color: bool,
    ) {
        // Build a full clap Command with all arguments

        let cmd = clap::Command::new(command_name.to_string())
            .about("Repository statistics and analysis tool")
            .version(env!("CARGO_PKG_VERSION"))
            .color(Self::color_choice(color, no_color))
            .arg(
                clap::Arg::new("repository")
                    .short('r')
                    .long("repo")
                    .value_name("PATH")
                    .help("Repository path to analyze (defaults to current directory)"),
            )
            .arg(
                clap::Arg::new("config-file")
                    .long("config-file")
                    .value_name("FILE")
                    .help("Configuration file path"),
            )
            .arg(
                clap::Arg::new("plugin-dir")
                    .long("plugin-dir")
                    .value_name("DIR")
                    .help("Plugin directory override"),
            )
            .arg(
                clap::Arg::new("plugin-exclude")
                    .long("plugin-exclude")
                    .value_name("LIST")
                    .help("Plugin exclusion list"),
            )
            .arg(
                clap::Arg::new("color")
                    .long("color")
                    .action(clap::ArgAction::SetTrue)
                    .help("Force colored output (overrides TTY detection and NO_COLOR)"),
            )
            .arg(
                clap::Arg::new("no-color")
                    .long("no-color")
                    .conflicts_with("color")
                    .action(clap::ArgAction::SetTrue)
                    .help("Disable colored output"),
            )
            .arg(
                clap::Arg::new("log-level")
                    .long("log-level")
                    .value_name("LEVEL")
                    .value_parser(["trace", "debug", "info", "warn", "error", "off"])
                    .help("Log level (trace, debug, info, warn, error, off)"),
            )
            .arg(
                clap::Arg::new("log-file")
                    .long("log-file")
                    .value_name("FILE")
                    .help("Log file path (use 'none' to disable file logging)"),
            )
            .arg(
                clap::Arg::new("log-format")
                    .long("log-format")
                    .value_name("FORMAT")
                    .value_parser(["text", "ext", "json"])
                    .help("Log output format (text, ext, json)"),
            );

        match cmd.try_get_matches_from(args) {
            Ok(matches) => {
                // Apply command line args (overrides the config file)
                Self::apply_command_line(margs, &matches);
            }
            Err(e) => {
                // Display help/error and exit
                e.print().expect("Error writing to stderr");
                std::process::exit(1);
            }
        }
    }

    /// Apply configuration file values to Args (async version)
    pub async fn parse_config_file_async(margs: &mut Self, config_file: Option<PathBuf>) {
        Self::parse_config_file_impl(margs, config_file).await;
    }

    /// Core implementation for config file parsing - now async
    async fn parse_config_file_impl(margs: &mut Self, config_file: Option<PathBuf>) {
        let config_path = match config_file {
            Some(path) => {
                // User specified a config file-it must exist
                if !path.exists() {
                    eprintln!(
                        "Error: The specified configuration file does not exist: {}",
                        path.display()
                    );
                    std::process::exit(1);
                }
                Some(path)
            }
            None => {
                // Use default config path if it exists
                let default_path =
                    dirs::config_dir().map(|d| d.join("Repostats").join("repostats.toml"));

                match default_path {
                    Some(path) if path.exists() => Some(path),
                    _ => None, // No config file to load
                }
            }
        };

        // If we have a config path, load and parse it
        if let Some(path) = config_path {
            match tokio::fs::read_to_string(&path).await {
                Ok(contents) => match toml::from_str::<toml::Table>(&contents) {
                    Ok(config) => Self::apply_toml_values(margs, &config),
                    Err(e) => {
                        eprintln!("Error parsing configuration file {}: {}", path.display(), e);
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("Error reading configuration file {}: {}", path.display(), e);
                    std::process::exit(1);
                }
            }
        }
    }

    /// Apply TOML configuration values to Args
    fn apply_toml_values(args: &mut Self, config: &toml::Table) {
        if let Some(repo) = config.get("repository").and_then(|v| v.as_str()) {
            args.repository = Some(PathBuf::from(repo));
        }
        if let Some(plugin_dir) = config.get("plugin-dir").and_then(|v| v.as_str()) {
            args.plugin_dir = Some(plugin_dir.to_string());
        }
        if let Some(plugin_exclude) = config.get("plugin-exclude").and_then(|v| v.as_str()) {
            args.plugin_exclude = Some(plugin_exclude.to_string());
        }
        if let Some(color) = config.get("color").and_then(|v| v.as_bool()) {
            args.color = color;
        }
        if let Some(no_color) = config.get("no-color").and_then(|v| v.as_bool()) {
            args.no_color = no_color;
        }
        if let Some(log_level) = config.get("log-level").and_then(|v| v.as_str()) {
            args.log_level = Some(log_level.to_string());
        }
        if let Some(log_file) = config.get("log-file").and_then(|v| v.as_str()) {
            if log_file != "none" {
                args.log_file = Some(PathBuf::from(log_file));
            } else {
                args.log_file = None;
            }
        }
        if let Some(log_format) = config.get("log-format").and_then(|v| v.as_str()) {
            args.log_format = Some(log_format.to_string());
        }
    }

    /// Apply command line arguments to Args (overrides config file values)
    fn apply_command_line(args: &mut Self, matches: &clap::ArgMatches) {
        if let Some(repo) = matches.get_one::<String>("repository") {
            args.repository = Some(PathBuf::from(repo));
        }
        if let Some(config_file) = matches.get_one::<String>("config-file") {
            args.config_file = Some(PathBuf::from(config_file));
        }
        if let Some(plugin_dir) = matches.get_one::<String>("plugin-dir") {
            args.plugin_dir = Some(plugin_dir.clone());
        }
        if let Some(plugin_exclude) = matches.get_one::<String>("plugin-exclude") {
            args.plugin_exclude = Some(plugin_exclude.clone());
        }
        if matches.get_flag("color") {
            args.color = true;
        }
        if matches.get_flag("no-color") {
            args.no_color = true;
        }
        if let Some(log_level) = matches.get_one::<String>("log-level") {
            args.log_level = Some(log_level.clone());
        }
        if let Some(log_file) = matches.get_one::<String>("log-file") {
            if log_file == "none" {
                args.log_file = None; // Magic "none" value disables file logging
            } else {
                args.log_file = Some(PathBuf::from(log_file));
            }
        }
        if let Some(log_format) = matches.get_one::<String>("log-format") {
            args.log_format = Some(log_format.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static COMMAND_NAME: &str = "repostats";

    #[test]
    fn test_parse_initial_stops_at_command() {
        let args = vec![
            "repostats".to_string(),
            "--log-level".to_string(),
            "debug".to_string(),
            "--color".to_string(),
            "metrics".to_string(), // This is a command - stop here
            "--help".to_string(),
        ];

        let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args);

        assert_eq!(parsed.log_level, Some("debug".to_string()));
        assert!(parsed.color);
        assert_eq!(
            global_args,
            vec![
                "repostats".to_string(),
                "--log-level".to_string(),
                "debug".to_string(),
                "--color".to_string(),
            ]
        );
    }

    #[test]
    fn test_parse_initial_stops_at_first_non_flag() {
        let args = vec![
            "repostats".to_string(),
            "--log-file".to_string(),
            "path".to_string(),
            "metrics".to_string(), // Command - should stop here
            "--help".to_string(),  // This belongs to the command
        ];

        let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args);

        assert_eq!(parsed.log_file, Some(PathBuf::from("path")));
        assert_eq!(
            global_args,
            vec![
                "repostats".to_string(),
                "--log-file".to_string(),
                "path".to_string(),
            ]
        );
    }

    #[test]
    fn test_parse_initial_handles_equals_format() {
        let args = vec![
            "repostats".to_string(),
            "--log-level=info".to_string(),
            "--color".to_string(),
            "scan".to_string(),
            "--since".to_string(),
            "1week".to_string(),
        ];

        let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args);

        assert_eq!(parsed.log_level, Some("info".to_string()));
        assert!(parsed.color);
        assert_eq!(
            global_args,
            vec![
                "repostats".to_string(),
                "--log-level=info".to_string(),
                "--color".to_string(),
            ]
        );
    }

    #[test]
    fn test_parse_initial_handles_log_file_none() {
        let args = vec![
            "repostats".to_string(),
            "--log-file".to_string(),
            "none".to_string(),
            "debug".to_string(), // Command
        ];

        let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args);

        assert_eq!(parsed.log_file, None); // "none" becomes None
        assert_eq!(
            global_args,
            vec![
                "repostats".to_string(),
                "--log-file".to_string(),
                "none".to_string(),
            ]
        );
    }

    #[test]
    fn test_parse_with_repository() {
        let args = vec![
            "repostats".to_string(),
            "--repo".to_string(),
            "/path/to/repo".to_string(),
        ];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(result.repository, Some(PathBuf::from("/path/to/repo")));
    }

    #[test]
    fn test_conflicting_args() {
        // Color and no-color still conflict
        let args = vec![
            "repostats".to_string(),
            "--color".to_string(),
            "--no-color".to_string(),
        ];

        let result = Args::try_parse_from(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_all_fields() {
        let args = vec![
            "repostats".to_string(),
            "--config-file".to_string(),
            "custom.toml".to_string(),
            "--plugin-dir".to_string(),
            "/plugins".to_string(),
            "--plugin-exclude".to_string(),
            "bad-plugin".to_string(),
            "--repo".to_string(),
            "/path/to/repo".to_string(),
            "--log-level".to_string(),
            "debug".to_string(),
            "--color".to_string(),
        ];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(result.config_file, Some(PathBuf::from("custom.toml")));
        assert_eq!(result.plugin_dir, Some("/plugins".to_string()));
        assert_eq!(result.plugin_exclude, Some("bad-plugin".to_string()));
        assert_eq!(result.repository, Some(PathBuf::from("/path/to/repo")));
        assert_eq!(result.log_level, Some("debug".to_string()));
        assert!(result.color);
        assert!(!result.no_color);
    }

    #[test]
    fn test_parse_initial_handles_equals_syntax() {
        let args = vec![
            "repostats".to_string(),
            "--log-level=debug".to_string(),
            "--config-file=/path/to/config.toml".to_string(),
            "--color".to_string(),
            "commits".to_string(), // Command
            "--since".to_string(),
            "1week".to_string(),
        ];

        let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args);

        assert_eq!(parsed.log_level, Some("debug".to_string()));
        assert_eq!(
            parsed.config_file,
            Some(PathBuf::from("/path/to/config.toml"))
        );
        assert_eq!(parsed.color, true);
        assert_eq!(
            global_args,
            vec![
                "repostats",
                "--log-level=debug",
                "--config-file=/path/to/config.toml",
                "--color"
            ]
        );
    }

    #[test]
    fn test_parse_initial_with_mixed_args() {
        // Test parsing with both --flag=value and --flag value formats
        let args = vec![
            "repostats".to_string(),
            "--log-level=debug".to_string(),
            "--config-file".to_string(),
            "/path/config.toml".to_string(),
            "--color".to_string(),
            "metrics".to_string(),
            "--stats".to_string(),
        ];

        let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args);

        assert_eq!(parsed.log_level, Some("debug".to_string()));
        assert_eq!(parsed.config_file, Some(PathBuf::from("/path/config.toml")));
        assert_eq!(parsed.color, true);
        assert_eq!(
            global_args,
            vec![
                "repostats",
                "--log-level=debug",
                "--config-file",
                "/path/config.toml",
                "--color"
            ]
        );
    }
}
