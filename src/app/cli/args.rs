//! Global arguments parsing for the full command line interface
//!
//! This module handles the complete parsing of global arguments after segmentation.
//! It uses standard clap parsing since we have clean global args at this stage.

use clap::{ArgAction, Parser};
use std::collections::HashSet;
use std::path::PathBuf;

// Global arguments structure with all command-line options
//
// Used for both initial parsing (before command discovery) and final parsing
// (after command segmentation). Handles all global configuration flags.
#[derive(Parser, Debug, Clone)]
#[command(name = "repostats")]
#[command(about = "Repository statistics and analysis tool")]
#[command(version)]
#[command(after_help = " * can be specified multiple times or as a comma-separated list")]
pub struct Args {
    /// Repositories to analyze*
    #[arg(short = 'r', long = "repo", value_name = "PATHS/URLS")]
    pub repository: Vec<PathBuf>,

    /// Configuration file path
    #[arg(short = 'c', long = "config-file", value_name = "FILE")]
    pub config_file: Option<PathBuf>,

    /// Plugin directory override
    #[arg(short = 'p', long = "plugin-dir", value_name = "DIR")]
    pub plugin_dir: Option<String>,

    /// Plugins to exclude from discovery*
    #[arg(long = "exclude-plugin", value_name = "NAMES", action = ArgAction::Append)]
    pub plugin_exclusions: Vec<String>,

    /// Force colored output (overrides TTY detection and NO_COLOR)
    #[arg(short = 'g', long = "color")]
    pub color: bool,

    /// Disable colored output
    #[arg(short = 'n', long = "no-color", conflicts_with = "color")]
    pub no_color: bool,

    /// Log level
    #[arg(short = 'l', long = "log-level", value_name = "LEVEL", value_parser = ["trace", "debug", "info", "warn", "error", "off"])]
    pub log_level: Option<String>,

    /// Log file path (use 'none' to disable file logging)
    #[arg(
        short = 'f',
        long = "log-file",
        value_name = "FILE",
        help = "Log file path (use 'none' to disable file logging)"
    )]
    pub log_file: Option<PathBuf>,

    /// Log output format
    #[arg(short = 'o', long = "log-format", value_name = "FORMAT", value_parser = ["text", "ext", "json"])]
    pub log_format: Option<String>,

    /// Start date/time for filtering (ISO 8601 or relative)
    #[arg(
        short = 'S',
        long = "since",
        value_name = "DATE_TIME",
        help = "Start date/time (YYYY-MM-DD, past: 'yesterday', '1 week ago', or future: 'in 2 days')"
    )]
    pub since: Option<String>,

    /// End date/time for filtering (ISO 8601 or relative)
    #[arg(
        short = 'U',
        long = "until",
        value_name = "DATE_TIME",
        help = "End date/time (YYYY-MM-DD, past: 'today', '1 hour ago', or future: '2 hours from now')"
    )]
    pub until: Option<String>,

    /// Authors to include* (matches name or email containing @)
    #[arg(short = 'A', long = "author", value_name = "AUTHORS")]
    pub author: Vec<String>,

    /// Authors to exclude* (matches name or email containing @)
    #[arg(long = "exclude-author", value_name = "AUTHORS")]
    pub exclude_author: Vec<String>,

    /// File patterns to include*
    #[arg(short = 'F', long = "files", value_name = "PATTERNS", action = ArgAction::Append)]
    pub files: Vec<String>,

    /// File patterns to exclude*
    #[arg(long = "exclude-files", value_name = "PATTERNS", action = ArgAction::Append)]
    pub exclude_files: Vec<String>,

    /// Include only files in these paths*
    #[arg(short = 'P', long = "paths", value_name = "PATHS", action = ArgAction::Append)]
    pub paths: Vec<String>,

    /// File extensions to include*
    #[arg(short = 'X', long = "extensions", value_name = "EXTS", action = ArgAction::Append)]
    pub extensions: Vec<String>,

    /// File extensions to exclude*
    #[arg(long = "exclude-extensions", value_name = "EXTS", action = ArgAction::Append)]
    pub exclude_extensions: Vec<String>,

    /// Paths to exclude*
    #[arg(short = 'N', long = "exclude-paths", value_name = "PATHS", action = ArgAction::Append)]
    pub exclude_paths: Vec<String>,

    /// Git reference to analyze (branch, tag, commit SHA, or HEAD)
    #[arg(short = 'R', long = "ref", value_name = "REF")]
    pub git_ref: Option<String>,

    /// Maximum number of commits to analyze
    #[arg(short = 'C', long = "max-commits", value_name = "COUNT")]
    pub max_commits: Option<usize>,

    /// Exclude merge commits from analysis
    #[arg(
        short = 'M',
        long = "no-merge-commits",
        conflicts_with = "merge_commits"
    )]
    pub no_merge_commits: bool,

    /// Include merge commits in analysis (overrides config file)
    #[arg(long = "merge-commits", conflicts_with = "no_merge_commits")]
    pub merge_commits: bool,

    /// Maximum files changed per commit
    #[arg(short = 'L', long = "max-files-per-commit", value_name = "COUNT")]
    pub max_files_per_commit: Option<usize>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            repository: Vec::new(),
            config_file: None,
            plugin_dir: None,
            plugin_exclusions: Vec::new(),
            color: false,
            no_color: false,
            log_level: None,
            log_file: None,
            log_format: Some("text".to_string()), // Default format
            since: None,
            until: None,
            author: Vec::new(),
            exclude_author: Vec::new(),
            files: Vec::new(),
            exclude_files: Vec::new(),
            paths: Vec::new(),
            extensions: Vec::new(),
            exclude_extensions: Vec::new(),
            exclude_paths: Vec::new(),
            git_ref: None,
            max_commits: None,
            no_merge_commits: false,
            merge_commits: false,
            max_files_per_commit: None,
        }
    }
}

/// Field type for determining appropriate parsing method in TOML configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldType {
    /// Fields containing file paths (require path validation)
    PathField,
    /// Fields containing regular strings (no path validation)
    StringField,
}

impl Args {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply enhanced parsing to handle comma-separated values, deduplication, and path validation
    pub fn apply_enhanced_parsing(&mut self) -> Result<(), String> {
        self.repository = Self::parse_comma_separated_paths(&self.repository);
        self.plugin_exclusions = Self::parse_comma_separated_strings(&self.plugin_exclusions);
        self.author = Self::parse_comma_separated_strings(&self.author);
        self.exclude_author = Self::parse_comma_separated_strings(&self.exclude_author);
        self.files = Self::parse_comma_separated_path_patterns(&self.files)?;
        self.exclude_files = Self::parse_comma_separated_path_patterns(&self.exclude_files)?;
        self.paths = Self::parse_comma_separated_path_patterns(&self.paths)?;
        self.extensions = Self::parse_comma_separated_strings(&self.extensions);
        self.exclude_extensions = Self::parse_comma_separated_strings(&self.exclude_extensions);
        self.exclude_paths = Self::parse_comma_separated_path_patterns(&self.exclude_paths)?;
        Ok(())
    }

    /// Parse comma-separated paths from a vector of PathBufs
    fn parse_comma_separated_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
        let mut result = Vec::new();

        for path in paths {
            let path_str = path.to_string_lossy();
            // Known and accepted limitation: no support for paths containing a comma
            if path_str.contains(',') {
                // Split by comma and trim whitespace
                for split_path in path_str.split(',') {
                    let trimmed = split_path.trim();
                    if !trimmed.is_empty() {
                        result.push(PathBuf::from(trimmed));
                    }
                }
            } else {
                // No comma, use the path as-is
                result.push(path.clone());
            }
        }

        result
    }

    /// Parse comma-separated strings from a vector of Strings with deduplication
    fn parse_comma_separated_strings(strings: &[String]) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for string in strings {
            if string.contains(',') {
                // Split by comma and trim whitespace
                for split_string in string.split(',') {
                    let trimmed = split_string.trim().to_string();
                    if !trimmed.is_empty() && !seen.contains(&trimmed) {
                        seen.insert(trimmed.clone());
                        result.push(trimmed);
                    }
                }
            } else {
                // No comma, use the string as-is
                let trimmed = string.trim().to_string();
                if !trimmed.is_empty() && !seen.contains(&trimmed) {
                    seen.insert(trimmed.clone());
                    result.push(trimmed);
                }
            }
        }

        result
    }

    /// Process a list of patterns, deduplicating and validating paths
    /// This deals with paths of files in a git commit, not filesystem paths
    fn parse_comma_separated_path_patterns(strings: &[String]) -> Result<Vec<String>, String> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for string in strings {
            if string.contains(',') {
                // Split by comma and trim whitespace
                for split_string in string.split(',') {
                    let trimmed = split_string.trim();
                    if !trimmed.is_empty() {
                        let normalized = Self::normalize_path_pattern(trimmed)?;
                        if !seen.contains(&normalized) {
                            seen.insert(normalized.clone());
                            result.push(normalized);
                        }
                    }
                }
            } else {
                // No comma, use the string as-is
                let trimmed = string.trim();
                if !trimmed.is_empty() {
                    let normalized = Self::normalize_path_pattern(trimmed)?;
                    if !seen.contains(&normalized) {
                        seen.insert(normalized.clone());
                        result.push(normalized);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Normalize path patterns, rejecting absolute paths with a clear error
    fn normalize_path_pattern(pattern: &str) -> Result<String, String> {
        if pattern.starts_with('/') {
            Err(format!(
                "Absolute paths are not supported in path patterns: '{}'. Please use a relative path.",
                pattern
            ))
        } else {
            Ok(pattern.to_string())
        }
    }

    /// Determine if colors should be used based on settings
    fn should_use_colors(color: bool, no_color: bool) -> bool {
        if color {
            true
        } else if no_color || std::env::var("NO_COLOR").is_ok() {
            false
        } else {
            std::io::IsTerminal::is_terminal(&std::io::stdout())
        }
    }

    /// Get colored asterisk string for help text
    fn get_colored_star(effective_color: bool) -> String {
        use clap::builder::styling::{AnsiColor, Color, Style};

        if effective_color {
            let style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
            // `style` prints the ANSI sequence, `{:#}` prints the reset code
            format!("{style}*{:#}", style)
        } else {
            "*".to_string()
        }
    }

    /// Build the complete clap Command with all argument definitions
    fn build_clap_command(
        command_name: &str,
        color_enabled: bool,
        color_choice: clap::ColorChoice,
        allow_external_subcommands: bool,
        ignore_errors: bool,
        include_help_version: bool,
    ) -> clap::Command {
        let mut cmd = clap::Command::new(command_name.to_string())
            .about("Repository statistics and analysis tool")
            .version(env!("CARGO_PKG_VERSION"))
            .after_help(Self::get_after_help(color_enabled))
            .disable_version_flag(true)
            .disable_help_flag(true)
            .styles(Self::get_help_styles(color_enabled))
            .help_template("{before-help}{name} {version}\n{author-with-newline}{about-with-newline}\n{usage-heading} {usage}\n\n{all-args}{after-help}")
            .override_usage("repostats [OPTIONS] [COMMAND]");

        // Apply color configuration - always set to respect NO_COLOR environment variable
        let star = Self::get_colored_star(color_enabled);
        cmd = cmd.color(color_choice);

        // Apply external subcommands configuration
        if allow_external_subcommands {
            cmd = cmd.allow_external_subcommands(true);
        }

        // Apply error handling configuration
        if ignore_errors {
            cmd = cmd.ignore_errors(true);
        }

        // Add argument definitions - non-filter options first (lowercase short forms)
        let mut cmd = cmd
            // Repository and configuration options
            .arg(
                clap::Arg::new("repository")
                    .short('r')
                    .long("repo")
                    .value_name("PATHS/URLS")
                    .value_parser(clap::value_parser!(PathBuf))
                    .action(ArgAction::Append)
                    .help(&format!("{} Repositories to analyze", star)),
            )
            .arg(
                clap::Arg::new("config_file")
                    .short('c')
                    .long("config-file")
                    .value_name("FILE")
                    .value_parser(clap::value_parser!(PathBuf))
                    .help("Configuration file path"),
            )
            .arg(
                clap::Arg::new("plugin_dir")
                    .short('p')
                    .long("plugin-dir")
                    .value_name("DIR")
                    .help("Plugin directory override"),
            )
            .arg(
                clap::Arg::new("plugin_exclusions")
                    .long("exclude-plugin")
                    .value_name("NAMES")
                    .action(ArgAction::Append)
                    .help(&format!("{} Plugins to exclude from discovery", star)),
            )
            // Output and display options
            .arg(
                clap::Arg::new("color")
                    .short('g')
                    .long("color")
                    .action(ArgAction::SetTrue)
                    .help("Force colored output (overrides TTY detection and NO_COLOR)"),
            )
            .arg(
                clap::Arg::new("no_color")
                    .short('n')
                    .long("no-color")
                    .conflicts_with("color")
                    .action(ArgAction::SetTrue)
                    .help("Disable colored output"),
            )
            // Logging options
            .arg(
                clap::Arg::new("log_level")
                    .short('l')
                    .long("log-level")
                    .value_name("LEVEL")
                    .value_parser(["trace", "debug", "info", "warn", "error", "off"])
                    .help("Log level"),
            )
            .arg(
                clap::Arg::new("log_format")
                    .short('o')
                    .long("log-format")
                    .value_name("FORMAT")
                    .value_parser(["text", "ext", "json"])
                    .help("Log output format"),
            )
            .arg(
                clap::Arg::new("log_file")
                    .short('f')
                    .long("log-file")
                    .value_name("FILE")
                    .value_parser(clap::value_parser!(PathBuf))
                    .help("Log file path (use 'none' to disable file logging)"),
            )
            // Filter options (uppercase short forms)
            .arg(
                clap::Arg::new("author")
                    .short('A')
                    .long("author")
                    .value_name("AUTHORS")
                    .action(ArgAction::Append)
                    .help(&format!("{} Authors to include (matches name or email containing @)", star)),
            )
            .arg(
                clap::Arg::new("exclude_author")
                    .long("exclude-author")
                    .value_name("AUTHORS")
                    .action(ArgAction::Append)
                    .help(&format!("{} Authors to exclude (matches name or email containing @)", star)),
            )
            .arg(
                clap::Arg::new("files")
                    .short('F')
                    .long("files")
                    .value_name("PATTERNS")
                    .action(ArgAction::Append)
                    .help(&format!("{} File patterns to include", star)),
            )
            .arg(
                clap::Arg::new("exclude_files")
                    .long("exclude-files")
                    .value_name("PATTERNS")
                    .action(ArgAction::Append)
                    .help(&format!("{} File patterns to exclude", star)),
            )
            .arg(
                clap::Arg::new("paths")
                    .short('P')
                    .long("paths")
                    .value_name("PATHS")
                    .action(ArgAction::Append)
                    .help(&format!("{} Include only files in these paths", star)),
            )
            .arg(
                clap::Arg::new("exclude_paths")
                    .short('N')
                    .long("exclude-paths")
                    .value_name("PATHS")
                    .action(ArgAction::Append)
                    .help(&format!("{} Exclude files in these paths", star)),
            )
            .arg(
                clap::Arg::new("extensions")
                    .short('X')
                    .long("extensions")
                    .value_name("EXTS")
                    .action(ArgAction::Append)
                    .help(&format!("{} Include only files with these extensions", star)),
            )
            .arg(
                clap::Arg::new("exclude_extensions")
                    .long("exclude-extensions")
                    .value_name("EXTS")
                    .action(ArgAction::Append)
                    .help(&format!("{} Exclude files with these extensions", star)),
            )
            .arg(
                clap::Arg::new("git_ref")
                    .short('R')
                    .long("ref")
                    .value_name("REF")
                    .help("Git reference to analyze (branch, tag, commit SHA, or HEAD)"),
            )
            .arg(
                clap::Arg::new("since")
                    .short('S')
                    .long("since")
                    .value_name("DATE_TIME")
                    .help("Start date/time (YYYY-MM-DD, past: 'yesterday', '1 week ago', or future: 'in 2 days')"),
            )
            .arg(
                clap::Arg::new("until")
                    .short('U')
                    .long("until")
                    .value_name("DATE_TIME")
                    .help("End date/time (YYYY-MM-DD, past: 'today', '1 hour ago', or future: '2 hours from now')"),
            )
            .arg(
                clap::Arg::new("max_commits")
                    .short('C')
                    .long("max-commits")
                    .value_name("NUM")
                    .value_parser(clap::value_parser!(usize))
                    .help("Maximum number of commits to analyse"),
            )
            .arg(
                clap::Arg::new("no_merge_commits")
                    .short('M')
                    .long("no-merge-commits")
                    .conflicts_with("merge_commits")
                    .action(ArgAction::SetTrue)
                    .help("Exclude merge commits from analysis"),
            )
            .arg(
                clap::Arg::new("merge_commits")
                    .long("merge-commits")
                    .conflicts_with("no_merge_commits")
                    .action(ArgAction::SetTrue)
                    .help("Include merge commits in analysis (overrides config file)"),
            )
            .arg(
                clap::Arg::new("max_files_per_commit")
                    .short('L')
                    .long("max-files-per-commit")
                    .value_name("NUM")
                    .value_parser(clap::value_parser!(usize))
                    .help("Maximum files changed per commit"),
            );

        // Add help/version args if requested (only in parse_from_args)
        if include_help_version {
            cmd = cmd
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
        }

        cmd
    }

    /// Parse initial global arguments using clap, stopping at first external subcommand
    ///
    /// Uses the same clap-based approach as parse_from_args but configured for initial parsing.
    /// Stops at the first external subcommand (assumed to be a command) and returns both
    /// the parsed arguments and the consumed argument list.
    pub fn parse_initial(command_name: &str, args: &[String]) -> (Self, Vec<String>) {
        use clap::FromArgMatches;

        if args.is_empty() {
            return (Self::default(), Vec::new());
        }

        // Call the helper method with initial parsing configuration (no help/version, no color styling)
        let cmd = Self::build_clap_command(
            command_name,
            false, // No color configuration for initial parse
            clap::ColorChoice::Auto,
            true,  // Allow external subcommands
            true,  // Ignore errors
            false, // Don't include help/version
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
            Err(_) => {
                // parse_initial only cares about essential arguments for plugin loading
                // Ignore any unknown arguments and let parse_from_args handle all validation
                (Self::default(), args[1..].to_vec())
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

    /// Get after-help text with colored asterisk if colors are enabled
    fn get_after_help(color_enabled: bool) -> String {
        let star = Self::get_colored_star(color_enabled);
        format!("{star} can be specified multiple times or as a comma-separated list")
    }

    /// Get help styles for colored output based on color settings
    fn get_help_styles(colors_enabled: bool) -> clap::builder::Styles {
        use clap::builder::styling::{AnsiColor, Color, Style};

        if !colors_enabled {
            clap::builder::Styles::plain()
        } else {
            // Create styled output with enhanced asterisk highlighting
            clap::builder::Styles::styled()
                .usage(
                    Style::new()
                        .bold()
                        .underline()
                        .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
                )
                .header(
                    Style::new()
                        .bold()
                        .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
                )
                // .literal(
                // Style::new()
                //     .fg_color(Some(Color::Ansi(AnsiColor::)))
                // )
                .invalid(
                    Style::new()
                        .bold()
                        .fg_color(Some(Color::Ansi(AnsiColor::Red))),
                )
                .error(
                    Style::new()
                        .bold()
                        .fg_color(Some(Color::Ansi(AnsiColor::Red))),
                )
                .valid(
                    Style::new()
                        .bold()
                        .fg_color(Some(Color::Ansi(AnsiColor::Green))),
                )
                .placeholder(
                    Style::new()
                        .bold()
                        .fg_color(Some(Color::Ansi(AnsiColor::BrightCyan))), // Brown-ish alternative
                )
        }
    }

    /// Parse global arguments from a provided argument list
    pub fn parse_from_args(
        margs: &mut Self,
        command_name: &str,
        args: &[String],
        color: bool,
        no_color: bool,
    ) {
        let effective_color = Self::should_use_colors(color, no_color);
        let color_choice = Self::color_choice(color, no_color);

        // Use the helper method with standard parsing configuration
        let cmd = Self::build_clap_command(
            command_name,
            effective_color, // Apply color configuration
            color_choice,
            false, // No external subcommands
            false, // Don't ignore errors
            true,  // Include help/version
        );

        match cmd.try_get_matches_from(args) {
            Ok(matches) => {
                // Apply command line args (overrides the config file)
                Self::apply_command_line(margs, &matches);
                // Apply enhanced parsing for comma-separated values
                if let Err(e) = margs.apply_enhanced_parsing() {
                    eprintln!("Error in argument validation: {}", e);
                    std::process::exit(1);
                }
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
                    Ok(config) => {
                        if let Err(e) = Self::apply_toml_values(margs, &config) {
                            eprintln!(
                                "Error in configuration file validation {}: {}",
                                path.display(),
                                e
                            );
                            std::process::exit(1);
                        }
                    }
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
    fn apply_toml_values(args: &mut Self, config: &toml::Table) -> Result<(), String> {
        if let Some(repo_value) = config.get("repository") {
            let mut repo_paths = Vec::new();

            if let Some(repo_str) = repo_value.as_str() {
                // Single repository format: repository = "path"
                repo_paths.push(PathBuf::from(repo_str));
            } else if let Some(repo_array) = repo_value.as_array() {
                // Array format: repository = ["path1", "path2"]
                for item in repo_array {
                    if let Some(path_str) = item.as_str() {
                        repo_paths.push(PathBuf::from(path_str));
                    }
                }
            }

            // Apply deduplication and add to existing repositories
            // Config values are added first, CLI args take precedence through later processing
            let deduplicated = Self::parse_comma_separated_paths(&repo_paths);
            args.repository.extend(deduplicated);
        }
        if let Some(plugin_dir) = config.get("plugin-dir").and_then(|v| v.as_str()) {
            args.plugin_dir = Some(plugin_dir.to_string());
        }
        
        // Handle plugin exclusions (support both single string and array formats)
        if let Some(exclusions_value) = config.get("exclude-plugin") {
            let mut exclusion_strings = Vec::new();

            if let Some(exclusion_str) = exclusions_value.as_str() {
                exclusion_strings.push(exclusion_str.to_string());
            } else if let Some(exclusion_array) = exclusions_value.as_array() {
                for item in exclusion_array {
                    if let Some(exclusion_str) = item.as_str() {
                        exclusion_strings.push(exclusion_str.to_string());
                    }
                }
            }

            let deduplicated = Self::parse_comma_separated_strings(&exclusion_strings);
            args.plugin_exclusions.extend(deduplicated);
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
            if log_file.eq_ignore_ascii_case("none") || log_file == "-" {
                args.log_file = None; // Magic values "none" and "-" disable file logging
            } else {
                args.log_file = Some(PathBuf::from(log_file));
            }
        }
        if let Some(log_format) = config.get("log-format").and_then(|v| v.as_str()) {
            args.log_format = Some(log_format.to_string());
        }
        if let Some(since) = config.get("since").and_then(|v| v.as_str()) {
            args.since = Some(since.to_string());
        }
        if let Some(until) = config.get("until").and_then(|v| v.as_str()) {
            args.until = Some(until.to_string());
        }

        // Handle author fields (support both single string and array formats)
        if let Some(author_value) = config.get("author") {
            let mut author_strings = Vec::new();

            if let Some(author_str) = author_value.as_str() {
                author_strings.push(author_str.to_string());
            } else if let Some(author_array) = author_value.as_array() {
                for item in author_array {
                    if let Some(author_str) = item.as_str() {
                        author_strings.push(author_str.to_string());
                    }
                }
            }

            let deduplicated = Self::parse_comma_separated_strings(&author_strings);
            args.author.extend(deduplicated);
        }

        if let Some(exclude_author_value) = config.get("exclude-author") {
            let mut author_strings = Vec::new();

            if let Some(author_str) = exclude_author_value.as_str() {
                author_strings.push(author_str.to_string());
            } else if let Some(author_array) = exclude_author_value.as_array() {
                for item in author_array {
                    if let Some(author_str) = item.as_str() {
                        author_strings.push(author_str.to_string());
                    }
                }
            }

            let deduplicated = Self::parse_comma_separated_strings(&author_strings);
            args.exclude_author.extend(deduplicated);
        }

        // Handle file filtering fields
        Self::apply_array_field(config, "files", &mut args.files)?;
        Self::apply_array_field(config, "exclude-files", &mut args.exclude_files)?;
        Self::apply_array_field(config, "paths", &mut args.paths)?;
        Self::apply_array_field(config, "exclude-paths", &mut args.exclude_paths)?;
        Self::apply_array_field(config, "extensions", &mut args.extensions)?;
        Self::apply_array_field(config, "exclude-extensions", &mut args.exclude_extensions)?;

        // Handle git reference
        if let Some(git_ref) = config.get("ref").and_then(|v| v.as_str()) {
            args.git_ref = Some(git_ref.to_string());
        }

        // Handle commit limits
        if let Some(max_commits) = config.get("max-commits").and_then(|v| v.as_integer()) {
            args.max_commits = Some(max_commits as usize);
        }
        if let Some(max_files) = config
            .get("max-files-per-commit")
            .and_then(|v| v.as_integer())
        {
            args.max_files_per_commit = Some(max_files as usize);
        }
        // Handle mutually exclusive merge commit flags from TOML
        if let Some(no_merge) = config.get("no-merge-commits").and_then(|v| v.as_bool()) {
            args.no_merge_commits = no_merge;
        } else if let Some(merge) = config.get("merge-commits").and_then(|v| v.as_bool()) {
            args.no_merge_commits = !merge;
        }

        Ok(())
    }

    /// Get the field type for a TOML configuration key
    fn get_field_type(key: &str) -> FieldType {
        match key {
            // Path-based fields that require path validation
            "files" | "exclude-files" | "paths" | "exclude-paths" => FieldType::PathField,
            // String-based fields that don't require path validation
            "author" | "exclude-author" | "extensions" | "exclude-extensions" => {
                FieldType::StringField
            }
            // Default to string field for unknown keys (safer default)
            _ => FieldType::StringField,
        }
    }

    /// Helper method to apply array field from TOML config with deduplication
    fn apply_array_field(
        config: &toml::Table,
        key: &str,
        target: &mut Vec<String>,
    ) -> Result<(), String> {
        if let Some(value) = config.get(key) {
            let mut temp_strings = Vec::new();

            if let Some(str_val) = value.as_str() {
                // Single string format
                temp_strings.push(str_val.to_string());
            } else if let Some(array_val) = value.as_array() {
                // Array format
                for item in array_val {
                    if let Some(item_str) = item.as_str() {
                        temp_strings.push(item_str.to_string());
                    }
                }
            }

            // Apply appropriate parsing based on explicit field type mapping
            let deduplicated = match Self::get_field_type(key) {
                FieldType::PathField => Self::parse_comma_separated_path_patterns(&temp_strings)?,
                FieldType::StringField => Self::parse_comma_separated_strings(&temp_strings),
            };

            target.extend(deduplicated);
        }
        Ok(())
    }

    /// Apply command line arguments to Args (overrides config file values)
    fn apply_command_line(args: &mut Self, matches: &clap::ArgMatches) {
        if let Some(repos) = matches.get_many::<PathBuf>("repository") {
            args.repository.extend(repos.cloned());
        }
        if let Some(config_file) = matches.get_one::<PathBuf>("config_file") {
            args.config_file = Some(config_file.clone());
        }
        if let Some(plugin_dir) = matches.get_one::<String>("plugin_dir") {
            args.plugin_dir = Some(plugin_dir.clone());
        }
        if let Some(plugin_exclusions) = matches.get_many::<String>("plugin_exclusions") {
            args.plugin_exclusions.extend(plugin_exclusions.cloned());
        }
        if matches.get_flag("color") {
            args.color = true;
        }
        if matches.get_flag("no_color") {
            args.no_color = true;
        }
        if let Some(log_level) = matches.get_one::<String>("log_level") {
            args.log_level = Some(log_level.clone());
        }
        if let Some(log_file) = matches.get_one::<PathBuf>("log_file") {
            let log_file_str = log_file.to_string_lossy();
            if log_file_str.eq_ignore_ascii_case("none") || log_file_str == "-" {
                args.log_file = None; // Magic values "none" and "-" disable file logging
            } else {
                args.log_file = Some(log_file.clone());
            }
        }
        if let Some(log_format) = matches.get_one::<String>("log_format") {
            args.log_format = Some(log_format.clone());
        }
        if let Some(since) = matches.get_one::<String>("since") {
            args.since = Some(since.clone());
        }
        if let Some(until) = matches.get_one::<String>("until") {
            args.until = Some(until.clone());
        }
        if let Some(authors) = matches.get_many::<String>("author") {
            args.author.extend(authors.cloned());
        }
        if let Some(exclude_authors) = matches.get_many::<String>("exclude_author") {
            args.exclude_author.extend(exclude_authors.cloned());
        }
        if let Some(files) = matches.get_many::<String>("files") {
            args.files.extend(files.cloned());
        }
        if let Some(exclude_files) = matches.get_many::<String>("exclude_files") {
            args.exclude_files.extend(exclude_files.cloned());
        }
        if let Some(paths) = matches.get_many::<String>("paths") {
            args.paths.extend(paths.cloned());
        }
        if let Some(exclude_paths) = matches.get_many::<String>("exclude_paths") {
            args.exclude_paths.extend(exclude_paths.cloned());
        }
        if let Some(extensions) = matches.get_many::<String>("extensions") {
            args.extensions.extend(extensions.cloned());
        }
        if let Some(exclude_extensions) = matches.get_many::<String>("exclude_extensions") {
            args.exclude_extensions.extend(exclude_extensions.cloned());
        }
        if let Some(git_ref) = matches.get_one::<String>("git_ref") {
            args.git_ref = Some(git_ref.clone());
        }
        if let Some(max_commits) = matches.get_one::<usize>("max_commits") {
            args.max_commits = Some(*max_commits);
        }
        if let Some(max_files_per_commit) = matches.get_one::<usize>("max_files_per_commit") {
            args.max_files_per_commit = Some(*max_files_per_commit);
        }
        // Mutually exclusive handling for merge commit flags
        if matches.get_flag("no_merge_commits") {
            args.no_merge_commits = true;
        } else if matches.get_flag("merge_commits") {
            args.no_merge_commits = false;
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

        assert_eq!(result.repository, vec![PathBuf::from("/path/to/repo")]);
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
            "--repo".to_string(),
            "/path/to/repo".to_string(),
            "--log-level".to_string(),
            "debug".to_string(),
            "--color".to_string(),
        ];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(result.config_file, Some(PathBuf::from("custom.toml")));
        assert_eq!(result.plugin_dir, Some("/plugins".to_string()));
        assert_eq!(result.repository, vec![PathBuf::from("/path/to/repo")]);
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

    #[test]
    fn test_multiple_repository_flags() {
        let args = vec![
            "repostats".to_string(),
            "--repo".to_string(),
            "/path/to/repo1".to_string(),
            "--repo".to_string(),
            "/path/to/repo2".to_string(),
            "-r".to_string(),
            "/path/to/repo3".to_string(),
        ];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(
            result.repository,
            vec![
                PathBuf::from("/path/to/repo1"),
                PathBuf::from("/path/to/repo2"),
                PathBuf::from("/path/to/repo3")
            ]
        );
    }

    #[test]
    fn test_empty_repository_list() {
        let args = vec!["repostats".to_string()];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(result.repository, Vec::<PathBuf>::new());
    }

    #[test]
    fn test_comma_separated_repository_parsing() {
        let args = vec![
            "repostats".to_string(),
            "--repo".to_string(),
            "/path/to/repo1,/path/to/repo2,/path/to/repo3".to_string(),
        ];

        let mut result = Args::try_parse_from(&args).unwrap();
        result.apply_enhanced_parsing().unwrap();

        assert_eq!(
            result.repository,
            vec![
                PathBuf::from("/path/to/repo1"),
                PathBuf::from("/path/to/repo2"),
                PathBuf::from("/path/to/repo3")
            ]
        );
    }

    #[test]
    fn test_mixed_repository_parsing() {
        let args = vec![
            "repostats".to_string(),
            "--repo".to_string(),
            "/single/repo".to_string(),
            "--repo".to_string(),
            "/comma/repo1,/comma/repo2".to_string(),
            "-r".to_string(),
            "/another/single".to_string(),
        ];

        let mut result = Args::try_parse_from(&args).unwrap();
        result.apply_enhanced_parsing().unwrap();

        assert_eq!(
            result.repository,
            vec![
                PathBuf::from("/single/repo"),
                PathBuf::from("/comma/repo1"),
                PathBuf::from("/comma/repo2"),
                PathBuf::from("/another/single")
            ]
        );
    }

    #[test]
    fn test_toml_single_repository() {
        use toml::Table;
        let mut args = Args::default();
        let mut config = Table::new();
        config.insert(
            "repository".to_string(),
            toml::Value::String("/path/to/single".to_string()),
        );

        Args::apply_toml_values(&mut args, &config).unwrap();

        assert_eq!(args.repository, vec![PathBuf::from("/path/to/single")]);
    }

    #[test]
    fn test_toml_array_repository() {
        use toml::Table;
        let mut args = Args::default();
        let mut config = Table::new();
        let repo_array = toml::Value::Array(vec![
            toml::Value::String("/path/to/repo1".to_string()),
            toml::Value::String("/path/to/repo2".to_string()),
            toml::Value::String("/path/to/repo3".to_string()),
        ]);
        config.insert("repository".to_string(), repo_array);

        Args::apply_toml_values(&mut args, &config).unwrap();

        assert_eq!(
            args.repository,
            vec![
                PathBuf::from("/path/to/repo1"),
                PathBuf::from("/path/to/repo2"),
                PathBuf::from("/path/to/repo3")
            ]
        );
    }

    #[test]
    fn test_date_filtering_args() {
        // Test with ISO 8601 dates
        let args_iso = vec![
            "repostats".to_string(),
            "--since".to_string(),
            "2024-01-01".to_string(),
            "--until".to_string(),
            "2024-12-31".to_string(),
        ];

        let result_iso = Args::try_parse_from(&args_iso).unwrap();
        assert_eq!(result_iso.since, Some("2024-01-01".to_string()));
        assert_eq!(result_iso.until, Some("2024-12-31".to_string()));

        // Test with relative dates
        let args_relative = vec![
            "repostats".to_string(),
            "--since".to_string(),
            "1 week ago".to_string(),
            "--until".to_string(),
            "yesterday".to_string(),
        ];

        let result_relative = Args::try_parse_from(&args_relative).unwrap();
        assert_eq!(result_relative.since, Some("1 week ago".to_string()));
        assert_eq!(result_relative.until, Some("yesterday".to_string()));
    }

    #[test]
    fn test_empty_date_filtering_args() {
        let args = vec!["repostats".to_string()];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(result.since, None);
        assert_eq!(result.until, None);
    }

    #[test]
    fn test_author_filtering_args() {
        let args = vec![
            "repostats".to_string(),
            "--author".to_string(),
            "john.doe@example.com".to_string(),
            "--author".to_string(),
            "jane.smith".to_string(),
            "--exclude-author".to_string(),
            "bot@example.com".to_string(),
        ];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(
            result.author,
            vec!["john.doe@example.com".to_string(), "jane.smith".to_string()]
        );
        assert_eq!(result.exclude_author, vec!["bot@example.com".to_string()]);
    }

    #[test]
    fn test_comma_separated_author_parsing() {
        let args = vec![
            "repostats".to_string(),
            "--author".to_string(),
            "john@example.com,jane@example.com,mike@example.com".to_string(),
        ];

        let mut result = Args::try_parse_from(&args).unwrap();
        result.apply_enhanced_parsing().unwrap();

        assert_eq!(
            result.author,
            vec![
                "john@example.com".to_string(),
                "jane@example.com".to_string(),
                "mike@example.com".to_string()
            ]
        );
    }

    #[test]
    fn test_file_filtering_args() {
        let args = vec![
            "repostats".to_string(),
            "--files".to_string(),
            "*.rs".to_string(),
            "--exclude-files".to_string(),
            "*.test.rs".to_string(),
            "--paths".to_string(),
            "src/".to_string(),
            "--extensions".to_string(),
            "rs".to_string(),
            "--exclude-extensions".to_string(),
            "tmp".to_string(),
        ];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(result.files, vec!["*.rs".to_string()]);
        assert_eq!(result.exclude_files, vec!["*.test.rs".to_string()]);
        assert_eq!(result.paths, vec!["src/".to_string()]);
        assert_eq!(result.extensions, vec!["rs".to_string()]);
        assert_eq!(result.exclude_extensions, vec!["tmp".to_string()]);
    }

    #[test]
    fn test_comma_separated_extension_parsing() {
        let args = vec![
            "repostats".to_string(),
            "--extensions".to_string(),
            "rs,toml,md".to_string(),
        ];

        let mut result = Args::try_parse_from(&args).unwrap();
        result.apply_enhanced_parsing().unwrap();

        assert_eq!(
            result.extensions,
            vec!["rs".to_string(), "toml".to_string(), "md".to_string()]
        );
    }

    #[test]
    fn test_path_deduplication_relative_paths() {
        let args = vec![
            "repostats".to_string(),
            "--files".to_string(),
            "src/*.rs".to_string(),
            "--files".to_string(),
            "src/*.rs".to_string(), // Duplicate should be removed
            "--paths".to_string(),
            "test/,src/,src/".to_string(), // Duplicates should be removed
        ];

        let mut result = Args::try_parse_from(&args).unwrap();
        result.apply_enhanced_parsing().unwrap();

        assert_eq!(result.files, vec!["src/*.rs".to_string()]);
        assert_eq!(result.paths, vec!["test/".to_string(), "src/".to_string()]);
    }

    #[test]
    fn test_absolute_path_rejection() {
        let args = vec![
            "repostats".to_string(),
            "--files".to_string(),
            "/src/*.rs".to_string(), // Absolute path should be rejected
        ];

        let mut result = Args::try_parse_from(&args).unwrap();
        let error = result.apply_enhanced_parsing().unwrap_err();
        assert!(error.contains("Absolute paths are not supported"));
        assert!(error.contains("/src/*.rs"));
    }

    #[test]
    fn test_absolute_path_rejection_in_comma_separated() {
        let args = vec![
            "repostats".to_string(),
            "--paths".to_string(),
            "relative/path,/absolute/path,another/relative".to_string(),
        ];

        let mut result = Args::try_parse_from(&args).unwrap();
        let error = result.apply_enhanced_parsing().unwrap_err();
        assert!(error.contains("Absolute paths are not supported"));
        assert!(error.contains("/absolute/path"));
    }

    #[test]
    fn test_cross_source_deduplication() {
        use toml::Table;
        let mut args = Args::default();

        // Simulate TOML config adding values
        let mut config = Table::new();
        let author_array = toml::Value::Array(vec![
            toml::Value::String("alice@example.com".to_string()),
            toml::Value::String("bob@example.com".to_string()),
        ]);
        config.insert("author".to_string(), author_array);
        Args::apply_toml_values(&mut args, &config).unwrap();

        // Simulate CLI args adding overlapping values
        args.author.push("bob@example.com".to_string()); // Duplicate from TOML
        args.author.push("charlie@example.com".to_string()); // New value
        args.author.push("alice@example.com".to_string()); // Another duplicate from TOML

        // Apply enhanced parsing to deduplicate
        args.apply_enhanced_parsing().unwrap();

        // Should contain each unique value only once, preserving order
        assert_eq!(
            args.author,
            vec![
                "alice@example.com".to_string(),
                "bob@example.com".to_string(),
                "charlie@example.com".to_string()
            ]
        );
    }

    #[test]
    fn test_cross_source_deduplication_with_comma_separated() {
        use toml::Table;
        let mut args = Args::default();

        // Simulate TOML config adding comma-separated values
        let mut config = Table::new();
        config.insert(
            "extensions".to_string(),
            toml::Value::String("rs,toml".to_string()),
        );
        Args::apply_toml_values(&mut args, &config).unwrap();

        // Simulate CLI args adding overlapping comma-separated values
        args.extensions.push("toml,md,rs".to_string()); // Overlaps with TOML values

        // Apply enhanced parsing to deduplicate
        args.apply_enhanced_parsing().unwrap();

        // Should contain each unique value only once, preserving order
        assert_eq!(
            args.extensions,
            vec!["rs".to_string(), "toml".to_string(), "md".to_string()]
        );
    }

    #[test]
    fn test_mutually_exclusive_merge_commit_flags() {
        // Test --no-merge-commits flag sets the field to true
        let args_no_merge = vec!["repostats".to_string(), "--no-merge-commits".to_string()];
        let result_no_merge = Args::try_parse_from(&args_no_merge).unwrap();
        assert!(result_no_merge.no_merge_commits);
        assert!(!result_no_merge.merge_commits);

        // Test --merge-commits flag sets the field to false
        let args_merge = vec!["repostats".to_string(), "--merge-commits".to_string()];
        let result_merge = Args::try_parse_from(&args_merge).unwrap();
        assert!(!result_merge.no_merge_commits);
        assert!(result_merge.merge_commits);

        // Test that using both flags together fails
        let args_both = vec![
            "repostats".to_string(),
            "--no-merge-commits".to_string(),
            "--merge-commits".to_string(),
        ];
        let result_both = Args::try_parse_from(&args_both);
        assert!(result_both.is_err());
    }

    #[test]
    fn test_merge_commits_cli_override_toml() {
        use toml::Table;
        let mut args = Args::default();

        // TOML config sets no-merge-commits to true
        let mut config = Table::new();
        config.insert("no-merge-commits".to_string(), toml::Value::Boolean(true));
        Args::apply_toml_values(&mut args, &config).unwrap();
        assert!(args.no_merge_commits);

        // CLI overrides with --merge-commits (should set no_merge_commits to false)
        let cli_args = vec!["repostats".to_string(), "--merge-commits".to_string()];
        let result = Args::try_parse_from(&cli_args).unwrap();

        assert!(!result.no_merge_commits); // --merge-commits should set this to false
        assert!(result.merge_commits); // The flag itself should be true
    }

    #[test]
    fn test_toml_merge_commits_flag() {
        use toml::Table;
        let mut args = Args::default();

        // Test merge-commits = true in TOML (should set no_merge_commits to false)
        let mut config = Table::new();
        config.insert("merge-commits".to_string(), toml::Value::Boolean(true));
        Args::apply_toml_values(&mut args, &config).unwrap();

        assert!(!args.no_merge_commits); // merge-commits = true should set no_merge_commits = false
    }

    #[test]
    fn test_toml_no_merge_commits_precedence() {
        use toml::Table;
        let mut args = Args::default();

        // Test that no-merge-commits takes precedence over merge-commits in TOML
        let mut config = Table::new();
        config.insert("no-merge-commits".to_string(), toml::Value::Boolean(true));
        config.insert("merge-commits".to_string(), toml::Value::Boolean(true)); // This should be ignored
        Args::apply_toml_values(&mut args, &config).unwrap();

        assert!(args.no_merge_commits); // no-merge-commits should take precedence
    }

    #[test]
    fn test_field_type_mapping() {
        // Test explicit field type mapping instead of fragile key-based logic
        assert_eq!(Args::get_field_type("files"), FieldType::PathField);
        assert_eq!(Args::get_field_type("exclude-files"), FieldType::PathField);
        assert_eq!(Args::get_field_type("paths"), FieldType::PathField);
        assert_eq!(Args::get_field_type("exclude-paths"), FieldType::PathField);

        assert_eq!(Args::get_field_type("author"), FieldType::StringField);
        assert_eq!(
            Args::get_field_type("exclude-author"),
            FieldType::StringField
        );
        assert_eq!(Args::get_field_type("extensions"), FieldType::StringField);
        assert_eq!(
            Args::get_field_type("exclude-extensions"),
            FieldType::StringField
        );

        // Test unknown keys default to StringField (safer default)
        assert_eq!(Args::get_field_type("unknown-key"), FieldType::StringField);
        assert_eq!(
            Args::get_field_type("some-path-key"),
            FieldType::StringField
        ); // Would fail with old logic
        assert_eq!(Args::get_field_type("files-backup"), FieldType::StringField);
        // Would fail with old logic
    }

    #[test]
    fn test_apply_array_field_with_explicit_mapping() {
        use toml::Table;
        let mut args = Args::default();
        let mut config = Table::new();

        // Test path field (should use path parsing with validation)
        config.insert(
            "files".to_string(),
            toml::Value::String("src/*.rs,test.rs".to_string()),
        );
        Args::apply_toml_values(&mut args, &config).unwrap();
        assert_eq!(
            args.files,
            vec!["src/*.rs".to_string(), "test.rs".to_string()]
        );

        // Test string field (should use string parsing without path validation)
        let mut config2 = Table::new();
        config2.insert(
            "extensions".to_string(),
            toml::Value::String("rs,toml,md".to_string()),
        );
        let mut args2 = Args::default();
        Args::apply_toml_values(&mut args2, &config2).unwrap();
        assert_eq!(
            args2.extensions,
            vec!["rs".to_string(), "toml".to_string(), "md".to_string()]
        );
    }

    #[test]
    fn test_plugin_exclusions_parsing() {
        let args = vec![
            "repostats".to_string(),
            "--exclude-plugin".to_string(),
            "dump".to_string(),
        ];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(result.plugin_exclusions, vec!["dump".to_string()]);
    }

    #[test]
    fn test_comma_separated_plugin_exclusions_parsing() {
        let args = vec![
            "repostats".to_string(),
            "--exclude-plugin".to_string(),
            "dump,plugin2,plugin3".to_string(),
        ];

        let mut result = Args::try_parse_from(&args).unwrap();
        result.apply_enhanced_parsing().unwrap();

        assert_eq!(
            result.plugin_exclusions,
            vec!["dump".to_string(), "plugin2".to_string(), "plugin3".to_string()]
        );
    }

    #[test]
    fn test_multiple_plugin_exclusion_flags() {
        let args = vec![
            "repostats".to_string(),
            "--exclude-plugin".to_string(),
            "dump".to_string(),
            "--exclude-plugin".to_string(),
            "plugin2".to_string(),
        ];

        let result = Args::try_parse_from(&args).unwrap();

        assert_eq!(result.plugin_exclusions, vec!["dump".to_string(), "plugin2".to_string()]);
    }

    #[test]
    fn test_field_type_mapping_prevents_path_validation_errors() {
        use toml::Table;
        let mut args = Args::default();
        let mut config = Table::new();

        // Test that string fields don't trigger path validation even with "/" characters
        // This would fail with absolute path validation if incorrectly classified as PathField
        config.insert(
            "author".to_string(),
            toml::Value::String("user@domain.com,/absolute/email/path".to_string()),
        );

        // This should succeed because "author" is correctly mapped to StringField
        let result = Args::apply_toml_values(&mut args, &config);
        assert!(result.is_ok());
        assert_eq!(
            args.author,
            vec![
                "user@domain.com".to_string(),
                "/absolute/email/path".to_string()
            ]
        );
    }

    #[test]
    fn test_path_field_validation_still_works() {
        use toml::Table;
        let mut args = Args::default();
        let mut config = Table::new();

        // Test that path fields still trigger validation and reject absolute paths
        config.insert(
            "files".to_string(),
            toml::Value::String("src/*.rs,/absolute/path.rs".to_string()),
        );

        // This should fail because "files" is correctly mapped to PathField and contains absolute path
        let result = Args::apply_toml_values(&mut args, &config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Absolute paths are not supported"));
    }
}
