//! CLI TOML configuration tests
//!
//! Tests for TOML configuration parsing, field type mapping, and CLI overrides.

use clap::Parser;
use repostats::app::cli::args::*;
use repostats::app::cli::config::FieldType;
use toml::Table;

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
    let mut args = Args::default();

    // Test merge-commits = true in TOML (should set no_merge_commits to false)
    let mut config = Table::new();
    config.insert("merge-commits".to_string(), toml::Value::Boolean(true));
    Args::apply_toml_values(&mut args, &config).unwrap();

    assert!(!args.no_merge_commits); // merge-commits = true should set no_merge_commits = false
}

#[test]
fn test_toml_no_merge_commits_precedence() {
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

    // Non-list fields should return StringField as default
    assert_eq!(Args::get_field_type("since"), FieldType::StringField);
    assert_eq!(Args::get_field_type("until"), FieldType::StringField);
    assert_eq!(Args::get_field_type("unknown"), FieldType::StringField);
}
