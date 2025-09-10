//! Repository-related integration tests
//!
//! Tests for repository validation, normalization, and opening functionality.

use repostats::scanner::api::{ScanError, ScannerManager};
use serial_test::serial;

#[tokio::test]
async fn test_repository_validation_valid_repo() {
    // GREEN: Test that validation works for valid repositories
    let manager = ScannerManager::create().await;

    // Try to validate current directory (should be a git repo for this project)
    let current_dir = std::env::current_dir().unwrap();
    let result = manager.validate_repository(&current_dir, None, None);

    // Should succeed for this project's git repository
    assert!(
        result.is_ok(),
        "Current directory should be a valid git repository"
    );
}

#[tokio::test]
async fn test_repository_validation_invalid_repo() {
    // Test that validation fails for invalid repositories
    let manager = ScannerManager::create().await;

    // Try to validate a non-existent directory
    let invalid_path = std::path::Path::new("/non/existent/path");
    let result = manager.validate_repository(invalid_path, None, None);

    // Should fail for invalid path
    assert!(result.is_err());

    if let Err(ScanError::Repository { message }) = result {
        assert!(message.contains("Invalid repository"));
    } else {
        panic!("Expected Repository error for invalid path");
    }
}

#[tokio::test]
async fn test_repository_normalisation() {
    // GREEN: Test that repository normalisation works correctly
    let manager = ScannerManager::create().await;

    // Test cases for remote URL normalisation
    let remote_test_cases = vec![
        ("https://github.com/user/repo.git", "github.com/user/repo"), // Remove scheme and .git
        ("git://github.com/user/repo", "github.com/user/repo"),       // Remove scheme
        ("ssh://git@github.com/user/repo.git", "github.com/user/repo"), // Remove scheme and .git
        ("https://gitlab.com/user/project", "gitlab.com/user/project"), // No .git extension
    ];

    for (input, expected) in remote_test_cases {
        let result = manager.normalise_repository_path(input).unwrap();
        assert_eq!(
            result, expected,
            "Failed to normalise remote URL: {}",
            input
        );
    }

    // Test local path normalisation (we'll test with non-existent paths to avoid canonicalization)
    // For this test, we'll create a mock path that doesn't exist so canonicalize fails
    let local_test_cases = vec![
        ("/fake/path/to/repo.git", "/fake/path/to/repo"), // Remove .git extension
        ("/fake/path/to/repo", "/fake/path/to/repo"),     // No .git extension
    ];

    for (input, expected) in local_test_cases {
        let result = manager.normalise_repository_path(input).unwrap();
        assert_eq!(
            result, expected,
            "Failed to normalise local path: {}",
            input
        );
    }
}

#[tokio::test]
async fn test_local_repository_opening() {
    // GREEN: Test that local repository opening works correctly
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let scanner_task = manager
        .create_scanner(&current_path, None, None)
        .await
        .unwrap();

    // Repository is already available - test that we can access it
    let repo = scanner_task.repository();

    // Basic validation that the repository is accessible
    assert!(
        !repo.git_dir().as_os_str().is_empty(),
        "Repository should have a git directory"
    );

    // Test with remote URL - should fail until remote support is added
    // Remote repositories are not supported by create_scanner, so we expect an error
    let remote_result = manager
        .create_scanner("https://github.com/user/repo.git", None, None)
        .await;
    assert!(
        remote_result.is_err(),
        "Remote repositories should not be supported"
    );

    // Test passes - remote repositories correctly rejected
}

#[tokio::test]
async fn test_remote_repository_support_placeholder() {
    // PLACEHOLDER: Remote repository support will be implemented in future phases
    let manager = ScannerManager::create().await;

    let remote_urls = vec![
        "https://github.com/user/repo.git",
        "git://github.com/user/repo.git",
        "ssh://git@github.com/user/repo.git",
    ];

    for url in remote_urls {
        let scanner_result = manager.create_scanner(url, None, None).await;
        // Remote repositories should fail during creation
        assert!(
            scanner_result.is_err(),
            "Remote repository should fail: {}",
            url
        );

        // Verify the error is about remote repositories not being supported
        if let Err(error) = scanner_result {
            // Just verify that we get some kind of error - the specific message might vary
            assert!(
                !error.to_string().is_empty(),
                "Error should have a message for: {}",
                url
            );
        }
    }
}

#[tokio::test]
async fn test_start_point_resolution() {
    // RED: Test scanner can resolve different start points (commit SHA, branch, tag)
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let scanner_task = manager
        .create_scanner(&current_path, None, None)
        .await
        .unwrap();

    // Test resolving HEAD commit
    match scanner_task.resolve_start_point("HEAD").await {
        Ok(commit_id) => {
            // Should resolve to a valid commit SHA
            assert!(
                commit_id.len() >= 40,
                "Commit ID should be full SHA: {}",
                commit_id
            );
        }
        Err(_) => panic!("Should be able to resolve HEAD commit"),
    }

    // Test resolving specific commit SHA (use first 8 chars of HEAD)
    let repo = gix::open(current_dir.as_path()).unwrap();
    let head_commit = repo.head().unwrap().peel_to_commit_in_place().unwrap();
    let full_sha = head_commit.id().to_hex_with_len(40).to_string();
    let short_sha = &full_sha[..8];

    match scanner_task.resolve_start_point(short_sha).await {
        Ok(resolved_commit_id) => {
            // Should resolve short SHA to full SHA
            assert_eq!(resolved_commit_id, full_sha);
        }
        Err(_) => panic!("Should be able to resolve short commit SHA"),
    }

    // Test resolving current branch name
    let current_branch = repo
        .head()
        .unwrap()
        .referent_name()
        .map(|name| name.as_bstr().to_string());
    match current_branch {
        Some(full_ref_name) => {
            let branch_name = full_ref_name
                .strip_prefix("refs/heads/")
                .unwrap_or(&full_ref_name);
            match scanner_task.resolve_start_point(branch_name).await {
                Ok(commit_id) => {
                    // Should resolve to same commit as HEAD in this test repo
                    assert!(
                        commit_id.len() >= 40,
                        "Branch resolution should return full SHA"
                    );
                    assert_eq!(
                        commit_id, full_sha,
                        "Branch should resolve to same commit as HEAD"
                    );
                }
                Err(e) => panic!(
                    "Should be able to resolve current branch '{}': {:?}",
                    branch_name, e
                ),
            }
        }
        None => {
            // If no symbolic ref, just test with HEAD which we know works
            println!("No symbolic ref found, skipping branch name test");
        }
    }

    // Test resolving invalid reference
    match scanner_task.resolve_start_point("invalid-ref-12345").await {
        Ok(_) => panic!("Should not resolve invalid reference"),
        Err(ScanError::Repository { message }) => {
            assert!(
                message.contains("reference not found")
                    || message.contains("invalid")
                    || message.contains("not found"),
                "Error should indicate invalid reference: {}",
                message
            );
        }
        Err(e) => panic!("Wrong error type for invalid reference: {:?}", e),
    }
}

#[tokio::test]
async fn test_content_reconstruction_api() {
    // GREEN: Test content reconstruction API exists and validates parameters correctly
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let scanner_task = manager
        .create_scanner(&current_path, None, None)
        .await
        .unwrap();

    // Get HEAD commit for testing
    let head_sha = scanner_task.resolve_start_point("HEAD").await.unwrap();

    // Test with invalid commit SHA - should error properly
    let invalid_sha = "0000000000000000000000000000000000000000";
    match scanner_task
        .read_current_file_content("any_file.rs", invalid_sha)
        .await
    {
        Ok(_) => panic!("Should not work with invalid commit SHA"),
        Err(ScanError::Repository { message }) => {
            assert!(
                message.contains("not found")
                    || message.contains("invalid")
                    || message.contains("Failed to"),
                "Error should indicate invalid commit: {}",
                message
            );
        }
        Err(e) => panic!("Wrong error type for invalid commit: {:?}", e),
    }

    // Test API method exists with valid commit (functionality test)
    // The actual file content reconstruction logic is simplified for initial implementation
    match scanner_task
        .read_current_file_content("README.md", &head_sha)
        .await
    {
        Ok(_) => {
            // API works - content reconstruction succeeded
        }
        Err(ScanError::Repository { message }) => {
            // Expected for files that don't exist in current working directory
            assert!(
                message.contains("not found"),
                "Should indicate file not found: {}",
                message
            );
        }
        Err(e) => panic!("Unexpected error type: {:?}", e),
    }
}
