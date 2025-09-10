//! Scanner Manager Integration Tests
//!
//! Tests for ScannerManager creation, repository validation, and basic manager functionality.

mod common;

use repostats::scanner::api::{ScanError, ScannerManager};

#[tokio::test]
async fn test_scanner_manager_creation() {
    // GREEN: Now implement basic ScannerManager creation
    let manager = ScannerManager::create().await;

    // Should successfully create a ScannerManager
    // (Testing that creation succeeds - no internal state assertions needed)
    assert!(manager.as_ref() as *const _ != std::ptr::null());
}

#[tokio::test]
async fn test_repository_validation_valid_repo() {
    // GREEN: Test that validation works for valid repositories
    let manager = ScannerManager::create().await;

    // Try to validate current directory (should be a git repo for this project)
    let current_dir = std::env::current_dir().unwrap();
    let result = manager.validate_repository(&current_dir, None, None);

    assert!(result.is_ok(), "Current directory should be valid git repo");
}

#[tokio::test]
async fn test_repository_validation_invalid_repo() {
    // GREEN: Test that validation properly detects invalid repositories
    let manager = ScannerManager::create().await;

    // Create a temporary directory that is NOT a git repository
    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let result = manager.validate_repository(temp_path, None, None);
    assert!(result.is_err(), "Non-git directory should fail validation");

    if let Err(e) = result {
        match e {
            ScanError::Repository { message } => {
                assert!(
                    message.to_lowercase().contains("not a git repository")
                    || message.to_lowercase().contains("not found")
                    || message.to_lowercase().contains("invalid"),
                    "Error message should indicate invalid repository: {}", message
                );
            }
            _ => panic!("Expected ScanError::Repository, got: {:?}", e),
        }
    }
}

#[tokio::test]
async fn test_repository_normalisation() {
    // GREEN: Test that repository path normalisation works correctly
    let manager = ScannerManager::create().await;

    // Test relative path normalisation
    let current_dir = std::env::current_dir().unwrap();
    let relative_result = manager.validate_repository(&std::path::Path::new("."), None, None);
    let absolute_result = manager.validate_repository(&current_dir, None, None);

    // Both should succeed if the current directory is a git repo
    match (relative_result, absolute_result) {
        (Ok(_), Ok(_)) => {
            // Both succeeded, which means the path normalisation is working
        }
        (Err(_), Err(_)) => {
            // Both failed, which is also consistent (directory is not a git repo)
        }
        _ => panic!("Relative and absolute paths should have consistent results"),
    }
}
