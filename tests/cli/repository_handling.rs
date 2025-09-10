//! CLI repository handling tests
//!
//! Tests for repository argument parsing, multiple repositories, and TOML configuration.

use clap::Parser;
use repostats::app::cli::args::*;
use std::path::PathBuf;
use toml::Table;

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
