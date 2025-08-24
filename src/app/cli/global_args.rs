//! Global arguments parsing for the full command line interface
//!
//! This module handles the complete parsing of global arguments after segmentation.
//! It uses standard clap parsing since we have clean global args at this stage.

use clap::Parser;
use std::path::PathBuf;

/// Global arguments structure with all command-line options
///
/// This is parsed AFTER segmentation, so we have clean global args
/// without any command-specific arguments mixed in.
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

    /// Cache directory
    #[arg(long = "cache-dir", value_name = "DIR")]
    pub cache_dir: Option<PathBuf>,

    /// Disable caching
    #[arg(long = "no-cache")]
    pub no_cache: bool,

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
    #[arg(long = "log-format", value_name = "FORMAT", value_parser = ["text", "json"])]
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
            cache_dir: None,
            no_cache: false,
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

    /// Parse global arguments from a provided argument list
    ///
    pub fn parse_from_args(margs: &mut Self, args: &[String], color: bool, no_color: bool) {
        // Build a full clap Command with all arguments
        let color_choice = if color {
            clap::ColorChoice::Always
        } else if no_color {
            clap::ColorChoice::Never
        } else {
            clap::ColorChoice::Auto
        };

        let cmd = clap::Command::new("repostats")
            .about("Repository statistics and analysis tool")
            .version(env!("CARGO_PKG_VERSION"))
            .color(color_choice)
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
                clap::Arg::new("cache-dir")
                    .long("cache-dir")
                    .value_name("DIR")
                    .help("Cache directory"),
            )
            .arg(
                clap::Arg::new("no-cache")
                    .long("no-cache")
                    .action(clap::ArgAction::SetTrue)
                    .help("Disable caching"),
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
                    .value_parser(["text", "json"])
                    .help("Log output format (text, json)"),
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
        if let Some(cache_dir) = config.get("cache-dir").and_then(|v| v.as_str()) {
            args.cache_dir = Some(PathBuf::from(cache_dir));
        }
        if let Some(no_cache) = config.get("no-cache").and_then(|v| v.as_bool()) {
            args.no_cache = no_cache;
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
        if let Some(cache_dir) = matches.get_one::<String>("cache-dir") {
            args.cache_dir = Some(PathBuf::from(cache_dir));
        }
        if matches.get_flag("no-cache") {
            args.no_cache = true;
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
            "--cache-dir".to_string(),
            "/cache".to_string(),
            "--log-level".to_string(),
            "debug".to_string(),
            "--color".to_string(),
        ];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(result.config_file, Some(PathBuf::from("custom.toml")));
        assert_eq!(result.plugin_dir, Some("/plugins".to_string()));
        assert_eq!(result.plugin_exclude, Some("bad-plugin".to_string()));
        assert_eq!(result.repository, Some(PathBuf::from("/path/to/repo")));
        assert_eq!(result.cache_dir, Some(PathBuf::from("/cache")));
        assert_eq!(result.log_level, Some("debug".to_string()));
        assert!(result.color);
        assert!(!result.no_color);
    }
}
