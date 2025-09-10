//! CLI filtering tests
//!
//! Tests for date filtering, author filtering, and file filtering argument parsing.

use clap::Parser;
use repostats::app::cli::args::*;

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
