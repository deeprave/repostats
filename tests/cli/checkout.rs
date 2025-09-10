//! CLI checkout functionality tests
//!
//! Tests for checkout directory parsing, flags, and related validation.

use clap::Parser;
use repostats::app::cli::args::*;

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
