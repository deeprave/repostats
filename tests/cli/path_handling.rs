//! CLI path handling tests
//!
//! Tests for path deduplication, absolute path validation, and cross-source deduplication.

use clap::Parser;
use repostats::app::cli::args::*;
use toml::Table;

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
    assert!(error.details().contains("Absolute paths are not supported"));
    assert!(error.details().contains("/src/*.rs"));
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
    assert!(error.details().contains("Absolute paths are not supported"));
    assert!(error.details().contains("/absolute/path"));
}

#[test]
fn test_cross_source_deduplication() {
    let mut args = Args::default();

    // Simulate TOML config adding values
    let mut config = Table::new();
    let author_array = toml::Value::Array(vec![
        toml::Value::String("alice@example.com".to_string()),
        toml::Value::String("bob@example.com".to_string()),
        toml::Value::String("alice@example.com".to_string()), // Duplicate
    ]);
    config.insert("author".to_string(), author_array);

    Args::apply_toml_values(&mut args, &config).unwrap();

    // Add CLI values with some duplicates
    args.author.push("charlie@example.com".to_string());
    args.author.push("alice@example.com".to_string()); // Duplicate from TOML

    args.apply_enhanced_parsing().unwrap();

    // Should deduplicate across both sources
    let mut expected = vec![
        "alice@example.com".to_string(),
        "bob@example.com".to_string(),
        "charlie@example.com".to_string(),
    ];
    expected.sort();

    let mut actual = args.author.clone();
    actual.sort();

    assert_eq!(actual, expected);
}

#[test]
fn test_cross_source_deduplication_with_comma_separated() {
    let mut args = Args::default();

    // Simulate TOML config
    let mut config = Table::new();
    let extension_array = toml::Value::Array(vec![
        toml::Value::String("rs".to_string()),
        toml::Value::String("toml".to_string()),
    ]);
    config.insert("extensions".to_string(), extension_array);

    Args::apply_toml_values(&mut args, &config).unwrap();

    // Add CLI values with comma-separated format
    args.extensions.push("rs,md,toml".to_string()); // Contains duplicates

    args.apply_enhanced_parsing().unwrap();

    // Should deduplicate across both sources
    let mut expected = vec!["rs".to_string(), "toml".to_string(), "md".to_string()];
    expected.sort();

    let mut actual = args.extensions.clone();
    actual.sort();

    assert_eq!(actual, expected);
}
