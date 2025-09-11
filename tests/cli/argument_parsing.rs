//! CLI argument parsing tests
//!
//! Tests for command-line argument parsing, initial parsing, and format handling.

use clap::Parser;
use repostats::app::cli::args::*;
use std::path::PathBuf;

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

    let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args).unwrap();

    assert_eq!(parsed.log_level, Some("debug".to_string()));
    assert_eq!(parsed.color, Some(true));
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

    let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args).unwrap();

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

    let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args).unwrap();

    assert_eq!(parsed.log_level, Some("info".to_string()));
    assert_eq!(parsed.color, Some(true));
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

    let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args).unwrap();

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
fn test_parse_all_fields() {
    let args = vec![
        "repostats".to_string(),
        "--config-file".to_string(),
        "custom.toml".to_string(),
        "--plugin-dirs".to_string(),
        "/plugins".to_string(),
        "--repo".to_string(),
        "/path/to/repo".to_string(),
        "--log-level".to_string(),
        "debug".to_string(),
    ];

    let result = Args::try_parse_from(&args).unwrap();

    assert_eq!(result.config_file, Some(PathBuf::from("custom.toml")));
    assert_eq!(result.plugin_dirs, vec!["/plugins".to_string()]);
    assert_eq!(result.repository, vec![PathBuf::from("/path/to/repo")]);
    assert_eq!(result.log_level, Some("debug".to_string()));
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

    let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args).unwrap();

    assert_eq!(parsed.log_level, Some("debug".to_string()));
    assert_eq!(
        parsed.config_file,
        Some(PathBuf::from("/path/to/config.toml"))
    );
    assert_eq!(parsed.color, Some(true));
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

    let (parsed, global_args) = Args::parse_initial(COMMAND_NAME, &args).unwrap();

    assert_eq!(parsed.log_level, Some("debug".to_string()));
    assert_eq!(parsed.config_file, Some(PathBuf::from("/path/config.toml")));
    assert_eq!(parsed.color, Some(true));
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
