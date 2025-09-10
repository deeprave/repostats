//! CLI plugin configuration tests
//!
//! Tests for plugin exclusion parsing and field type validation.

use clap::Parser;
use repostats::app::cli::args::*;
use toml::Table;

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
        vec![
            "dump".to_string(),
            "plugin2".to_string(),
            "plugin3".to_string()
        ]
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

    assert_eq!(
        result.plugin_exclusions,
        vec!["dump".to_string(), "plugin2".to_string()]
    );
}

#[test]
fn test_field_type_mapping_prevents_path_validation_errors() {
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
        .details()
        .contains("Absolute paths are not supported"));
}
