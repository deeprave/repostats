//! Tests for CLI arguments parsing, validation, and TOML integration

use crate::app::cli::args::*;
use crate::app::cli::config::FieldType;
use clap::Parser;
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
        "--plugin-dir".to_string(),
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
        .details()
        .contains("Absolute paths are not supported"));
}

#[test]
fn test_checkout_dir_parsing() {
    let args = vec![
        "repostats".to_string(),
        "--checkout-dir".to_string(),
        "/tmp/checkout-{repo}-{commit-id}".to_string(),
    ];

    let result = Args::try_parse_from(&args).unwrap();
    assert_eq!(
        result.checkout_dir,
        Some("/tmp/checkout-{repo}-{commit-id}".to_string())
    );
}

#[test]
fn test_checkout_keep_flags() {
    // Test --checkout-keep
    let args_keep = vec!["repostats".to_string(), "--checkout-keep".to_string()];
    let result_keep = Args::try_parse_from(&args_keep).unwrap();
    assert!(result_keep.checkout_keep);
    assert!(!result_keep.no_checkout_keep);

    // Test --no-checkout-keep
    let args_no_keep = vec!["repostats".to_string(), "--no-checkout-keep".to_string()];
    let result_no_keep = Args::try_parse_from(&args_no_keep).unwrap();
    assert!(!result_no_keep.checkout_keep);
    assert!(result_no_keep.no_checkout_keep);

    // Test mutual exclusion
    let args_both = vec![
        "repostats".to_string(),
        "--checkout-keep".to_string(),
        "--no-checkout-keep".to_string(),
    ];
    let result_both = Args::try_parse_from(&args_both);
    assert!(result_both.is_err());
}

#[test]
fn test_checkout_force_flag() {
    let args = vec!["repostats".to_string(), "--checkout-force".to_string()];

    let result = Args::try_parse_from(&args).unwrap();
    assert!(result.checkout_force);
}

#[test]
fn test_checkout_rev_parsing() {
    let args = vec![
        "repostats".to_string(),
        "--checkout-rev".to_string(),
        "main".to_string(),
    ];

    let result = Args::try_parse_from(&args).unwrap();
    assert_eq!(result.checkout_rev, Some("main".to_string()));

    // Test with commit SHA
    let args_sha = vec![
        "repostats".to_string(),
        "--checkout-rev".to_string(),
        "abc123def".to_string(),
    ];

    let result_sha = Args::try_parse_from(&args_sha).unwrap();
    assert_eq!(result_sha.checkout_rev, Some("abc123def".to_string()));
}

#[test]
fn test_checkout_args_default_values() {
    let result = Args::default();

    assert_eq!(result.checkout_dir, None);
    assert!(!result.checkout_keep);
    assert!(!result.no_checkout_keep);
    assert!(!result.checkout_force);
    assert_eq!(result.checkout_rev, None);
}

#[test]
fn test_validate_single_repository_with_checkout_success() {
    let mut args = Args::default();
    args.repository = vec!["https://github.com/user/repo.git".into()];
    args.checkout_dir = Some("/tmp/checkout".to_string());

    let result = args.validate();
    assert!(
        result.is_ok(),
        "Single repository with checkout should be valid"
    );
}

#[test]
fn test_validate_multiple_repositories_without_checkout_success() {
    let mut args = Args::default();
    args.repository = vec![
        "https://github.com/user/repo1.git".into(),
        "https://github.com/user/repo2.git".into(),
    ];
    // No checkout flags set

    let result = args.validate();
    assert!(
        result.is_ok(),
        "Multiple repositories without checkout should be valid"
    );
}

#[test]
fn test_validate_multiple_repositories_with_checkout_dir_error() {
    let mut args = Args::default();
    args.repository = vec![
        "https://github.com/user/repo1.git".into(),
        "https://github.com/user/repo2.git".into(),
    ];
    args.checkout_dir = Some("/tmp/checkout".to_string());

    let result = args.validate();
    assert!(
        result.is_err(),
        "Multiple repositories with --checkout-dir should fail"
    );
    let error = result.unwrap_err();
    assert!(error
        .details()
        .contains("Checkout functionality currently supports only a single repository"));
    assert!(error.details().contains("found 2"));
    // RS-29 is now in debug logs only, not user-facing error message
}

#[test]
fn test_validate_multiple_repositories_with_checkout_keep_error() {
    let mut args = Args::default();
    args.repository = vec![
        "https://github.com/user/repo1.git".into(),
        "https://github.com/user/repo2.git".into(),
    ];
    args.checkout_keep = true;

    let result = args.validate();
    assert!(
        result.is_err(),
        "Multiple repositories with --checkout-keep should fail"
    );
}

#[test]
fn test_validate_multiple_repositories_with_checkout_force_error() {
    let mut args = Args::default();
    args.repository = vec![
        "https://github.com/user/repo1.git".into(),
        "https://github.com/user/repo2.git".into(),
    ];
    args.checkout_force = true;

    let result = args.validate();
    assert!(
        result.is_err(),
        "Multiple repositories with --checkout-force should fail"
    );
}

#[test]
fn test_validate_multiple_repositories_with_checkout_rev_error() {
    let mut args = Args::default();
    args.repository = vec![
        "https://github.com/user/repo1.git".into(),
        "https://github.com/user/repo2.git".into(),
    ];
    args.checkout_rev = Some("main".to_string());

    let result = args.validate();
    assert!(
        result.is_err(),
        "Multiple repositories with --checkout-rev should fail"
    );
}

#[test]
fn test_validate_empty_repository_list_success() {
    let args = Args::default(); // repository list is empty by default

    let result = args.validate();
    assert!(
        result.is_ok(),
        "Empty repository list should be valid (defaults to current directory)"
    );
}

#[test]
fn test_validate_checkout_flags_without_dir_error() {
    let mut args = Args::default();
    args.repository = vec!["https://github.com/user/repo.git".into()];
    args.checkout_keep = true;
    // checkout_dir is None

    let result = args.validate();
    assert!(
        result.is_err(),
        "--checkout-keep without --checkout-dir should fail"
    );
    let error = result.unwrap_err();
    assert!(error.details().contains("require --checkout-dir"));
}

#[test]
fn test_validate_max_commits_zero_error() {
    let mut args = Args::default();
    args.repository = vec!["https://github.com/user/repo.git".into()];
    args.max_commits = Some(0);

    let result = args.validate();
    assert!(result.is_err(), "--max-commits 0 should fail validation");
    let error = result.unwrap_err();
    assert!(error.details().contains("must be greater than 0"));
}

#[test]
fn test_validate_max_commits_positive_success() {
    let mut args = Args::default();
    args.repository = vec!["https://github.com/user/repo.git".into()];
    args.max_commits = Some(100);

    let result = args.validate();
    assert!(
        result.is_ok(),
        "--max-commits with positive value should be valid"
    );
}

// Integration tests - verify end-to-end CLI validation functionality
mod integration_tests {
    use super::*;

    #[test]
    fn test_end_to_end_cli_validation_integration() {
        // Test that Args parsing + validation works together
        let mut args = Args::default();

        // Valid case: single repo, no checkout flags
        args.repository = vec!["https://github.com/user/repo.git".into()];
        assert!(args.validate().is_ok(), "Valid args should pass validation");

        // Invalid case: checkout flags with multiple repos (caught by validation)
        args.repository = vec![
            "https://github.com/user/repo1.git".into(),
            "https://github.com/user/repo2.git".into(),
        ];
        args.checkout_dir = Some("/tmp/checkout".to_string());

        let result = args.validate();
        assert!(result.is_err(), "Invalid config should fail validation");

        let error = result.unwrap_err();
        assert!(
            error.details().contains("only a single repository"),
            "Error should mention single repo restriction"
        );
        // RS-29 is now in debug logs only, not user-facing error message
    }

    #[test]
    fn test_validation_prevents_invalid_startup_configs() {
        // Simulate different invalid configurations that would be caught at startup
        // Note: Empty repository list is now valid (defaults to current directory)

        // Case 1: Checkout flags without checkout-dir
        let mut args_incomplete = Args::default();
        args_incomplete.repository = vec!["/repo".into()];
        args_incomplete.checkout_keep = true; // Missing checkout_dir
        assert!(
            args_incomplete.validate().is_err(),
            "Incomplete checkout config should fail"
        );

        // Case 2: Invalid max commits
        let mut args_bad_commits = Args::default();
        args_bad_commits.repository = vec!["/repo".into()];
        args_bad_commits.max_commits = Some(0);
        assert!(
            args_bad_commits.validate().is_err(),
            "Zero max commits should fail"
        );
    }

    #[test]
    fn test_complete_valid_configurations_pass() {
        // Test various valid configurations that should work end-to-end

        // Case 1: Remote repository URLs (should pass validation)
        let mut args_url = Args::default();
        args_url.repository = vec!["https://github.com/user/repo.git".into()];
        assert!(
            args_url.validate().is_ok(),
            "Remote repository URL should be valid"
        );

        // Case 2: Git SSH URL with checkout
        let mut args_ssh_checkout = Args::default();
        args_ssh_checkout.repository = vec!["git@github.com:user/repo.git".into()];
        args_ssh_checkout.checkout_dir = Some("/tmp/{commit-id}".to_string());
        args_ssh_checkout.checkout_keep = true;
        assert!(
            args_ssh_checkout.validate().is_ok(),
            "SSH repo with checkout should be valid"
        );

        // Case 3: Multiple remote repos without checkout
        let mut args_multi_url = Args::default();
        args_multi_url.repository = vec![
            "https://github.com/user/repo1.git".into(),
            "https://github.com/user/repo2.git".into(),
        ];
        assert!(
            args_multi_url.validate().is_ok(),
            "Multiple remote repos should be valid"
        );

        // Case 4: Empty repository list (should default to current directory)
        let args_empty = Args::default();
        assert!(
            args_empty.validate().is_ok(),
            "Empty repository list should be valid (defaults to current directory)"
        );
    }

    #[test]
    fn test_validate_local_git_repository() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary Git repository for testing
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let git_dir = temp_dir.path().join(".git");
        fs::create_dir(&git_dir).expect("Failed to create .git directory");

        // Test with valid Git repository
        let mut args_valid_git = Args::default();
        args_valid_git.repository = vec![temp_dir.path().to_path_buf()];
        assert!(
            args_valid_git.validate().is_ok(),
            "Valid Git repository should pass validation"
        );

        // Test with non-existent path
        let mut args_nonexistent = Args::default();
        args_nonexistent.repository = vec!["/this/path/does/not/exist".into()];
        let result = args_nonexistent.validate();
        assert!(
            result.is_err(),
            "Non-existent repository should fail validation"
        );
        assert!(result.unwrap_err().details().contains("does not exist"));

        // Test with existing directory but not a Git repository
        let non_git_dir = TempDir::new().expect("Failed to create temp directory");
        let mut args_not_git = Args::default();
        args_not_git.repository = vec![non_git_dir.path().to_path_buf()];
        let result_not_git = args_not_git.validate();
        assert!(
            result_not_git.is_err(),
            "Directory without .git should fail validation"
        );
        assert!(result_not_git
            .unwrap_err()
            .details()
            .contains("not a Git repository"));
    }

    #[test]
    fn test_validate_checkout_template_variables() {
        // Test valid checkout template
        let mut args_valid = Args::default();
        args_valid.repository = vec!["https://github.com/user/repo.git".into()];
        args_valid.checkout_dir = Some("/tmp/{repo}/{commit-id}".to_string());
        assert!(
            args_valid.validate().is_ok(),
            "Valid checkout template should pass"
        );

        // Test invalid checkout template with unknown variable
        let mut args_invalid = Args::default();
        args_invalid.repository = vec!["https://github.com/user/repo.git".into()];
        args_invalid.checkout_dir = Some("/tmp/{unknown-var}".to_string());
        let result = args_invalid.validate();
        assert!(
            result.is_err(),
            "Invalid checkout template should fail validation"
        );
        let error_msg = result.unwrap_err();
        assert!(error_msg
            .details()
            .contains("Invalid checkout directory template"));
        assert!(error_msg.details().contains("unknown-var"));

        // Test checkout template with all valid variables
        let mut args_all_vars = Args::default();
        args_all_vars.repository = vec!["https://github.com/user/repo.git".into()];
        args_all_vars.checkout_dir =
            Some("{tmpdir}/{repo}/{pid}-{scanner-id}-{commit-id}-{sha256}-{branch}".to_string());
        assert!(
            args_all_vars.validate().is_ok(),
            "Template with all variables should pass"
        );
    }

    #[test]
    fn test_validate_empty_repository_strings() {
        use std::path::PathBuf;

        // Test empty string in repository vector
        let mut args = Args::default();
        args.repository = vec![PathBuf::from("")];
        let result = args.validate();
        assert!(
            result.is_err(),
            "Empty repository string should fail validation"
        );
        assert!(result
            .unwrap_err()
            .details()
            .contains("Repository at index 0 cannot be empty"));

        // Test whitespace-only string in repository vector
        let mut args_ws = Args::default();
        args_ws.repository = vec![PathBuf::from("  \t\n  ")];
        let result_ws = args_ws.validate();
        assert!(
            result_ws.is_err(),
            "Whitespace-only repository string should fail validation"
        );
        assert!(result_ws
            .unwrap_err()
            .details()
            .contains("Repository at index 0 cannot be empty"));

        // Test mixed valid and invalid repositories
        let mut args_mixed = Args::default();
        args_mixed.repository = vec![
            PathBuf::from("https://github.com/user/repo.git"),
            PathBuf::from(""),
            PathBuf::from("https://github.com/user/repo2.git"),
        ];
        let result_mixed = args_mixed.validate();
        assert!(
            result_mixed.is_err(),
            "Mixed valid/invalid repositories should fail validation"
        );
        assert!(result_mixed
            .unwrap_err()
            .details()
            .contains("Repository at index 1 cannot be empty"));
    }

    #[test]
    fn test_macfs_case_sensitivity_flags() {
        // Test --macfs-case flag
        let args_case_sensitive = vec!["repostats".to_string(), "--macfs-case".to_string()];
        let result = Args::try_parse_from(&args_case_sensitive).unwrap();
        assert_eq!(result.macfs_case, Some(true));
        // After refactoring, the no_macfs_case field gets set to Some(false) by clap when not specified
        // This is different behavior but functionally equivalent since resolve_case_sensitivity_override() works correctly
        assert!(result.no_macfs_case.is_some() && !result.no_macfs_case.unwrap());

        // Test --no-macfs-case flag
        let args_case_insensitive = vec!["repostats".to_string(), "--no-macfs-case".to_string()];
        let result = Args::try_parse_from(&args_case_insensitive).unwrap();
        // After refactoring, macfs_case gets set to Some(false) when not specified
        assert!(result.macfs_case.is_some() && !result.macfs_case.unwrap());
        assert_eq!(result.no_macfs_case, Some(true));

        // Test mutual exclusion
        let args_both = vec![
            "repostats".to_string(),
            "--macfs-case".to_string(),
            "--no-macfs-case".to_string(),
        ];
        let result_both = Args::try_parse_from(&args_both);
        assert!(result_both.is_err());
    }

    #[test]
    fn test_resolve_case_sensitivity_override() {
        // Test --macfs-case resolves to Some(false) for case-sensitive
        let mut args = Args::default();
        args.macfs_case = Some(true);
        assert_eq!(args.resolve_case_sensitivity_override(), Some(false));

        // Test --no-macfs-case resolves to Some(true) for case-insensitive
        let mut args2 = Args::default();
        args2.no_macfs_case = Some(true);
        assert_eq!(args2.resolve_case_sensitivity_override(), Some(true));

        // Test no flags resolves to None for platform heuristic
        let args3 = Args::default();
        assert_eq!(args3.resolve_case_sensitivity_override(), None);
    }
}
