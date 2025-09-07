//! Core CLI arguments structure and basic functionality
//!
//! This module contains the main Args struct definition and basic methods.
//! Validation, parsing, and configuration loading are handled by separate modules.

use clap::{ArgAction, Parser};
use std::borrow::Cow;
use std::path::PathBuf;
use std::time::Duration;

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

    /// Color output control:
    /// --color sets Some(true), --no-color sets Some(false), unspecified = None (auto/TTY)
    #[arg(short = 'g', long = "color")]
    pub color: Option<bool>,

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
    #[arg(short = 'o', long = "log-format", value_name = "FORMAT", value_parser = ["text", "simple", "min", "ext", "json"])]
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

    /// List all discovered plugins and exit
    #[arg(
        long = "plugins",
        help = "List all discovered plugins and their functions"
    )]
    pub plugins: bool,

    /// Plugin operation timeout in seconds (minimum: 5)
    #[arg(long = "plugin-timeout", value_name = "SECONDS")]
    pub plugin_timeout: Option<u64>,

    /// Force case-sensitive filesystem handling on macOS (applies globally if before --repo)
    #[arg(long = "macfs-case", action = ArgAction::SetTrue)]
    pub macfs_case: Option<bool>,

    /// Force case-insensitive filesystem handling on macOS (applies globally if before --repo)
    #[arg(long = "no-macfs-case", action = ArgAction::SetTrue, conflicts_with = "macfs_case")]
    pub no_macfs_case: Option<bool>,

    /// Directory template for file checkout (supports {commit-id}, {sha256}, {branch}, {repo})
    #[arg(long = "checkout-dir", value_name = "DIRECTORY")]
    pub checkout_dir: Option<String>,

    /// Keep checked out files after processing (only valid with --checkout-dir)
    #[arg(long = "checkout-keep", conflicts_with = "no_checkout_keep")]
    pub checkout_keep: bool,

    /// Don't keep checked out files after processing (only valid with --checkout-dir)
    #[arg(long = "no-checkout-keep", conflicts_with = "checkout_keep")]
    pub no_checkout_keep: bool,

    /// Overwrite existing content in checkout directory
    #[arg(long = "checkout-force")]
    pub checkout_force: bool,

    /// Which revision to check out (default: HEAD)
    #[arg(long = "checkout-rev", value_name = "REV")]
    pub checkout_rev: Option<String>,
}

/// Settings for file checkout functionality
#[derive(Debug, Clone)]
pub struct CheckoutSettings {
    pub checkout_template: Option<String>,
    pub keep_checkouts: bool,
    pub force_overwrite: bool,
    pub default_revision: Option<String>,
}

impl Args {
    pub fn new() -> Self {
        Self::default()
    }

    /// Extract checkout settings from CLI arguments
    pub fn checkout_settings(&self) -> Option<CheckoutSettings> {
        if self.checkout_dir.is_none() {
            return None;
        }

        Some(CheckoutSettings {
            checkout_template: self.checkout_dir.clone(),
            keep_checkouts: self.checkout_keep && !self.no_checkout_keep,
            force_overwrite: self.checkout_force,
            default_revision: self.checkout_rev.clone(),
        })
    }

    /// Get normalized repository list with explicit default to current directory
    ///
    /// This method makes the default behavior explicit by converting empty repository
    /// lists to vec![PathBuf::from(".")] instead of relying on downstream defaulting.
    /// This eliminates hidden invariants and makes the behavior predictable.
    ///
    /// Returns a Cow to avoid cloning when the repository list is already populated.
    pub fn normalized_repositories(&self) -> Cow<'_, [PathBuf]> {
        if self.repository.is_empty() {
            Cow::Owned(vec![PathBuf::from(".")])
        } else {
            Cow::Borrowed(&self.repository)
        }
    }

    /// Resolve macOS filesystem case sensitivity override from CLI flags
    ///
    /// Returns the appropriate override value for ScannerManager construction:
    /// - Some(false) if --macfs-case is specified (force case-sensitive)
    /// - Some(true) if --no-macfs-case is specified (force case-insensitive)
    /// - None if neither flag is specified (use platform heuristic)
    pub fn resolve_case_sensitivity_override(&self) -> Option<bool> {
        match (self.macfs_case, self.no_macfs_case) {
            (Some(true), _) => Some(false), // --macfs-case = force case-sensitive (false = not case-insensitive)
            (_, Some(true)) => Some(true), // --no-macfs-case = force case-insensitive (true = case-insensitive)
            _ => None,                     // Neither flag specified, use platform heuristic
        }
    }

    /// Get plugin timeout as Duration (enforces minimum of 5 seconds)
    pub fn plugin_timeout_duration(&self) -> Duration {
        match self.plugin_timeout {
            Some(secs) => Duration::from_secs(secs.max(5)),
            None => Duration::from_secs(30), // Default 30 seconds
        }
    }
}

impl Default for Args {
    fn default() -> Self {
        Self {
            repository: Vec::new(),
            config_file: None,
            plugin_dir: None,
            plugin_exclusions: Vec::new(),
            color: None,
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
            plugins: false,
            plugin_timeout: None,
            macfs_case: None,
            no_macfs_case: None,
            checkout_dir: None,
            checkout_keep: false,
            no_checkout_keep: false,
            checkout_force: false,
            checkout_rev: None,
        }
    }
}
