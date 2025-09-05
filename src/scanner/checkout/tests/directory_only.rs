//! Directory-Only Operations Tests
//!
//! Tests for CheckoutManager after refactoring to remove Git operations
//! Following Single Responsibility Principle - CheckoutManager handles only directory operations

use super::super::*;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_prepare_checkout_directory_method() {
    // TDD test for directory-only CheckoutManager after refactoring
    // This should fail initially because prepare_checkout_directory doesn't exist yet

    let temp_base = TempDir::new().unwrap();
    let template = "/tmp/checkout-{scanner_id}-{commit_hash}".to_string();

    // Create CheckoutManager without repository dependency
    let mut manager = CheckoutManager::new(template, false);

    // Create template vars
    let vars = TemplateVars {
        scanner_id: "test-scanner".to_string(),
        commit_hash: "abc123def456".to_string(),
    };

    // TEST: This should fail because prepare_checkout_directory doesn't exist yet
    let checkout_dir = manager.prepare_checkout_directory(&vars).unwrap();

    // Verify directory was created
    assert!(checkout_dir.exists(), "Directory should be created");
    assert!(checkout_dir.is_dir(), "Should be a directory");

    // Verify path contains expected components
    let path_str = checkout_dir.to_string_lossy();
    assert!(path_str.contains("test-scanner"), "Should contain scanner_id");
    assert!(path_str.contains("abc123def456"), "Should contain commit_hash");

    // Verify it's tracked as an active checkout
    let checkout_id = format!("{}-{}", vars.scanner_id, vars.commit_hash);
    assert!(manager.is_checkout_active(&checkout_id), "Should track as active checkout");
}

#[tokio::test]
async fn test_directory_only_operations() {
    // Test that CheckoutManager can operate without Git dependencies
    let template = "/tmp/test-{scanner_id}".to_string();
    let mut manager = CheckoutManager::new(template, false);

    let vars1 = TemplateVars {
        scanner_id: "scanner1".to_string(),
        commit_hash: "commit1".to_string(),
    };

    let vars2 = TemplateVars {
        scanner_id: "scanner2".to_string(),
        commit_hash: "commit2".to_string(),
    };

    // Prepare multiple directories
    let dir1 = manager.prepare_checkout_directory(&vars1).unwrap();
    let dir2 = manager.prepare_checkout_directory(&vars2).unwrap();

    // Verify both exist and are different
    assert!(dir1.exists() && dir2.exists(), "Both directories should exist");
    assert_ne!(dir1, dir2, "Should create different directories");

    // Verify cleanup works
    let checkout_id1 = format!("{}-{}", vars1.scanner_id, vars1.commit_hash);
    manager.cleanup_checkout(&checkout_id1).unwrap();

    // Directory should be removed
    assert!(!dir1.exists(), "Directory should be removed after cleanup");
    assert!(dir2.exists(), "Other directory should remain");

    // Should no longer be tracked as active
    assert!(!manager.is_checkout_active(&checkout_id1), "Should not be active after cleanup");
}

#[tokio::test]
async fn test_force_overwrite_directory_logic() {
    // Test force_overwrite logic for directory operations
    let template = "/tmp/force-test-{scanner_id}".to_string();

    let vars = TemplateVars {
        scanner_id: "force-scanner".to_string(),
        commit_hash: "force-commit".to_string(),
    };

    // Test with force_overwrite = false
    let mut manager_no_force = CheckoutManager::new(template.clone(), false);
    let dir1 = manager_no_force.prepare_checkout_directory(&vars).unwrap();

    // Create a file in the directory
    std::fs::write(dir1.join("existing-file.txt"), "existing content").unwrap();

    // Attempting to prepare same directory again should handle existing directory
    let dir1_again = manager_no_force.prepare_checkout_directory(&vars).unwrap();
    assert_eq!(dir1, dir1_again, "Should handle existing directory");

    // Test with force_overwrite = true
    let mut manager_force = CheckoutManager::new(template, true);
    let dir2 = manager_force.prepare_checkout_directory(&vars).unwrap();

    // Should create/overwrite the directory
    assert!(dir2.exists(), "Directory should exist with force overwrite");
}
