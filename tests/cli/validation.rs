//! CLI validation tests
//!
//! Tests for argument validation, checkout restrictions, and configuration integrity.

use repostats::app::cli::args::*;

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
            "Invalid max commits should fail"
        );
    }
}
