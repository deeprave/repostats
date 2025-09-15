//! Command line argument parsing and enhanced parsing utilities
//!
//! This module handles clap-based CLI parsing, enhanced comma-separated value parsing,
//! and all parsing-related utilities for the command line interface.

use crate::core::validation::{split_and_collect, ValidationError};
use clap::ArgAction;
use std::path::PathBuf;

use super::args::Args;

impl Args {
    /// Apply enhanced parsing to handle comma-separated values, deduplication, and path validation
    pub fn apply_enhanced_parsing(&mut self) -> Result<(), ValidationError> {
        self.repository = Self::parse_comma_separated_paths(&self.repository);
        self.plugin_dirs = Self::parse_comma_separated_strings(&self.plugin_dirs);
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
    pub fn parse_comma_separated_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
        // Known limitation: no support for paths containing a comma
        // Use to_string_lossy() for filesystem paths as they may contain non-UTF-8 characters
        split_and_collect(paths, |path| path.to_string_lossy().to_string(), false)
            .into_iter()
            .map(PathBuf::from)
            .collect()
    }

    /// Parse comma-separated strings from a vector of Strings with deduplication
    pub fn parse_comma_separated_strings(strings: &[String]) -> Vec<String> {
        split_and_collect(strings, |s| s.clone(), true)
    }

    /// Process a list of patterns, deduplicating and validating paths
    /// This deals with paths of files in a git commit, not filesystem paths
    pub fn parse_comma_separated_path_patterns(
        strings: &[String],
    ) -> Result<Vec<String>, ValidationError> {
        let parts = split_and_collect(strings, |s| s.clone(), true);

        let mut normalized_parts = Vec::new();
        for part in &parts {
            let normalized = Self::normalize_path_pattern(part)?;
            normalized_parts.push(normalized);
        }

        Ok(normalized_parts)
    }

    /// Normalize path patterns, rejecting absolute paths with a clear error
    fn normalize_path_pattern(pattern: &str) -> Result<String, ValidationError> {
        if pattern.starts_with('/') {
            Err(ValidationError::new(&format!(
                "Absolute paths are not supported in path patterns: '{}'. Please use a relative path",
                pattern
            )))
        } else {
            Ok(pattern.to_string())
        }
    }

    /// Get colored asterisk string for help text (uses styles palette)
    fn get_colored_star(color_setting: Option<bool>) -> String {
        use crate::core::styles::StyleRole;
        // Resolve auto to TTY detection
        let enabled =
            color_setting.unwrap_or_else(|| std::io::IsTerminal::is_terminal(&std::io::stdout()));
        if enabled {
            StyleRole::Header.paint("*", enabled)
        } else {
            "*".to_string()
        }
    }

    /// Build the complete clap Command with all argument definitions
    fn build_clap_command(
        command_name: &str,
        color_enabled: Option<bool>,
        color_choice: clap::ColorChoice,
        allow_external_subcommands: bool,
        ignore_errors: bool,
        include_help_version: bool,
    ) -> clap::Command {
        let star = Self::get_colored_star(color_enabled);

        let mut cmd = clap::Command::new(command_name.to_string())
            .about("Repository statistics and analysis tool")
            .version(env!("CARGO_PKG_VERSION"))
            .disable_version_flag(true)
            .disable_help_flag(true)
            .help_template("{before-help}{name} {version}\n{author-with-newline}{about-with-newline}\n{usage-heading} {usage}\n\n{all-args}{after-help}")
            .override_usage("repostats [OPTIONS] COMMAND [ARGS]...")
            .after_help(Self::get_after_help(color_enabled))
            .color(color_choice)
            .styles(Self::get_help_styles(color_enabled.unwrap_or(false)))
            .ignore_errors(ignore_errors);

        // Apply external subcommands configuration
        if allow_external_subcommands {
            cmd = cmd.allow_external_subcommands(true);
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
                clap::Arg::new("plugin-dirs")
                    .short('p')
                    .long("plugin-dirs")
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
                    .overrides_with("no_color")
                    .help("Force colored output (overrides TTY detection and NO_COLOR)"),
            )
            .arg(
                clap::Arg::new("no_color")
                    .short('n')
                    .long("no-color")
                    .action(ArgAction::SetTrue)
                    .overrides_with("color")
                    .help("Force non-colored output (overrides FORCE_COLOR)"),
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
                    .value_parser(["text", "simple", "min", "ext", "json"])
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
            )
            .arg(
                clap::Arg::new("plugins")
                    .long("plugins")
                    .action(ArgAction::SetTrue)
                    .help("List all discovered plugins and their functions"),
            )
            .arg(
                clap::Arg::new("plugin_timeout")
                    .long("plugin_timeout")
                    .value_name("SECONDS")
                    .value_parser(clap::value_parser!(u64))
                    .help("Plugin operation timeout in seconds (minimum: 5, default: 30)"),
            )
            .arg(
                clap::Arg::new("macfs_case")
                    .long("macfs-case")
                    .action(ArgAction::SetTrue)
                    .help("Force case-sensitive filesystem handling on macOS (applies globally if before --repo)"),
            )
            .arg(
                clap::Arg::new("no_macfs_case")
                    .long("no-macfs-case")
                    .conflicts_with("macfs_case")
                    .action(ArgAction::SetTrue)
                    .help("Force case-insensitive filesystem handling on macOS (applies globally if before --repo)"),
            )
            .arg(
                clap::Arg::new("checkout_dir")
                    .long("checkout-dir")
                    .value_name("DIRECTORY")
                    .help("Directory template for file checkout (supports {commit-id}, {sha256}, {branch}, {repo})"),
            )
            .arg(
                clap::Arg::new("checkout_keep")
                    .long("checkout-keep")
                    .conflicts_with("no_checkout_keep")
                    .action(ArgAction::SetTrue)
                    .help("Keep checked out files after processing (only valid with --checkout-dir)"),
            )
            .arg(
                clap::Arg::new("no_checkout_keep")
                    .long("no-checkout-keep")
                    .conflicts_with("checkout_keep")
                    .action(ArgAction::SetTrue)
                    .help("Don't keep checked out files after processing (only valid with --checkout-dir)"),
            )
            .arg(
                clap::Arg::new("checkout_force")
                    .long("checkout-force")
                    .action(ArgAction::SetTrue)
                    .help("Overwrite existing content in checkout directory"),
            )
            .arg(
                clap::Arg::new("checkout_rev")
                    .long("checkout-rev")
                    .value_name("REV")
                    .help("Which revision to check out (default: HEAD)"),
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
    pub fn parse_initial(
        command_name: &str,
        args: &[String],
    ) -> Result<(Self, Vec<String>), ValidationError> {
        use clap::FromArgMatches;

        if args.is_empty() {
            return Ok((Self::default(), Vec::new()));
        }

        // Call the helper method with initial parsing configuration (no help/version, no color styling)
        let cmd = Self::build_clap_command(
            command_name,
            None, // No color configuration for initial parse (auto later)
            clap::ColorChoice::Auto,
            true,  // Allow external subcommands
            true,  // Ignore errors
            false, // Don't include help/version
        );

        match cmd.try_get_matches_from(args) {
            Ok(matches) => {
                // Parse using clap's standard mechanism with proper error handling
                let mut parsed_args = Self::from_arg_matches(&matches)
                    .map_err(|e| ValidationError::new(&format!("CLI parsing failed: {}", e)))?;

                parsed_args.color = if matches.value_source("color").is_some() {
                    Some(true)
                } else if matches.value_source("no_color").is_some() {
                    Some(false)
                } else {
                    None
                };

                // Handle special "none" value for log_file (same as parse_from_args)
                if let Some(ref path) = parsed_args.log_file {
                    if path.to_string_lossy() == "none" {
                        parsed_args.log_file = None;
                    }
                }

                // Extract the consumed arguments
                let global_args = Self::extract_consumed_args(args, &matches);
                Ok((parsed_args, global_args))
            }
            Err(e) => {
                // Propagate the error as a ValidationError instead of returning defaults
                Err(ValidationError::new(&format!(
                    "CLI argument parsing error: {}",
                    e
                )))
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

    fn color_choice(color_setting: Option<bool>) -> clap::ColorChoice {
        match color_setting {
            Some(true) => clap::ColorChoice::Always,
            Some(false) => clap::ColorChoice::Never,
            None => clap::ColorChoice::Auto,
        }
    }

    /// Get after-help text with colored asterisk if colors are enabled
    fn get_after_help(color_enabled: Option<bool>) -> String {
        let star = Self::get_colored_star(color_enabled);
        format!("{star} can be specified multiple times or as a comma-separated list")
    }

    /// Get help styles for colored output based on color settings
    fn get_help_styles(colors_enabled: bool) -> clap::builder::Styles {
        crate::core::styles::palette_to_clap(colors_enabled)
    }

    /// Parse global arguments from a provided argument list
    pub fn parse_from_args(
        margs: &mut Self,
        command_name: &str,
        args: &[String],
        color: Option<bool>,
    ) {
        let color_choice = Self::color_choice(color);

        // Use the helper method with standard parsing configuration
        let cmd = Self::build_clap_command(
            command_name,
            color,
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

    /// Apply command line arguments to Args (overrides config file values)
    fn apply_command_line(args: &mut Self, matches: &clap::ArgMatches) {
        if let Some(repos) = matches.get_many::<PathBuf>("repository") {
            args.repository.extend(repos.cloned());
        }
        if let Some(config_file) = matches.get_one::<PathBuf>("config_file") {
            args.config_file = Some(config_file.clone());
        }
        if let Some(plugin_dirs) = matches.get_many::<String>("plugin-dirs") {
            args.plugin_dirs.extend(plugin_dirs.cloned());
        }
        if let Some(plugin_exclusions) = matches.get_many::<String>("plugin_exclusions") {
            args.plugin_exclusions.extend(plugin_exclusions.cloned());
        }
        args.color = if matches.value_source("color").is_some() {
            Some(true)
        } else if matches.value_source("no_color").is_some() {
            Some(false)
        } else {
            None
        };
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

        // Plugin configuration
        if matches.get_flag("plugins") {
            args.plugins = true;
        }
        if let Some(timeout) = matches.get_one::<u64>("plugin_timeout") {
            args.plugin_timeout = Some(*timeout);
        }

        // macOS filesystem case sensitivity flags
        if matches.get_flag("macfs_case") {
            args.macfs_case = Some(true);
        }
        if matches.get_flag("no_macfs_case") {
            args.no_macfs_case = Some(true);
        }

        // Checkout arguments
        if let Some(checkout_dir) = matches.get_one::<String>("checkout_dir") {
            args.checkout_dir = Some(checkout_dir.clone());
        }
        if matches.get_flag("checkout_keep") {
            args.checkout_keep = true;
        }
        if matches.get_flag("no_checkout_keep") {
            args.no_checkout_keep = true;
        }
        if matches.get_flag("checkout_force") {
            args.checkout_force = true;
        }
        if let Some(checkout_rev) = matches.get_one::<String>("checkout_rev") {
            args.checkout_rev = Some(checkout_rev.clone());
        }
    }
}
