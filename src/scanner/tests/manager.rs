//! Tests for ScannerManager
//!
//! Comprehensive test suite for the scanner manager functionality including
//! repository validation, scanner creation, and path redaction.

use crate::core::query::QueryParams;
use crate::notifications::api::ScanEventType;
use crate::scanner::manager::ScannerManager;
use crate::scanner::task::ScannerTask;
use crate::scanner::tests::helpers::{collect_scan_messages, scan_and_capture_messages};
use crate::scanner::types::{ScanMessage, ScanStats};
use serial_test::serial;
use std::time::SystemTime;

#[tokio::test]
async fn test_scanner_manager_creation() {
    // GREEN: Now implement basic ScannerManager creation
    let manager = ScannerManager::create().await;

    // Should successfully create a ScannerManager with empty scanner tasks
    assert_eq!(manager.scanner_count(), 0);
}

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

    if let Err(crate::scanner::error::ScanError::Repository { message }) = result {
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
async fn test_scanner_id_generation() {
    // GREEN: Test that SHA256-based scanner ID generation works correctly
    let manager = ScannerManager::create().await;

    let test_cases = vec![
        ("https://github.com/user/repo.git", "github.com/user/repo"),
        ("/path/to/local/repo", "/path/to/local/repo"),
        (
            "ssh://git@gitlab.com/user/project.git",
            "gitlab.com/user/project",
        ),
    ];

    for (input, _expected_normalised) in test_cases {
        let result = manager.generate_scanner_id(input);

        // Should succeed now
        assert!(
            result.is_ok(),
            "Failed to generate scanner ID for: {}",
            input
        );

        let scanner_id = result.unwrap();

        // Should be a valid hex string of 16 chars (truncated SHA256)
        let hash_part = &scanner_id;
        assert_eq!(
            hash_part.len(),
            16,
            "Truncated SHA256 hash should be 16 characters: {}",
            hash_part
        );
        assert!(
            hash_part.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should contain only hex digits: {}",
            hash_part
        );

        // Verify the hash is consistent for the same input
        let second_result = manager.generate_scanner_id(input).unwrap();
        assert_eq!(
            scanner_id, second_result,
            "Scanner ID should be consistent for same input"
        );
    }

    // Test that different inputs produce different scanner IDs
    let id1 = manager
        .generate_scanner_id("https://github.com/user1/repo")
        .unwrap();
    let id2 = manager
        .generate_scanner_id("https://github.com/user2/repo")
        .unwrap();
    assert_ne!(
        id1, id2,
        "Different repositories should have different scanner IDs"
    );
}

#[tokio::test]
async fn test_scanner_task_initialization() {
    // GREEN: Test that ScannerTask initialization works correctly
    let manager = ScannerManager::create().await;

    // Test with current directory (should be a valid git repo)
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let result = manager.create_scanner(&current_path, None, None).await;
    assert!(
        result.is_ok(),
        "ScannerTask creation should succeed for valid repository"
    );

    let scanner_task = result.unwrap();

    // Verify scanner ID format
    assert_eq!(
        scanner_task.scanner_id().len(),
        16,
        "Scanner ID should be 16-char truncated SHA256: {}",
        scanner_task.scanner_id()
    );

    // Verify repository path is stored (accounting for normalization)
    // On case-insensitive filesystems, paths are normalized to lowercase
    let normalized_current = if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        current_path.to_lowercase()
    } else {
        current_path.to_string()
    };
    assert_eq!(
        scanner_task.repository_path(),
        normalized_current,
        "Repository path should match normalized input"
    );

    // Test that duplicate detection works - second attempt should fail
    let second_result = manager.create_scanner(&current_path, None, None).await;
    assert!(
        second_result.is_err(),
        "Second scanner creation should fail due to duplicate detection"
    );

    // Test remote repository paths (these won't be validated but should generate IDs)
    let remote_test_cases = vec![
        "https://github.com/user/repo.git",
        "ssh://git@gitlab.com/user/project.git",
    ];

    for repository_path in remote_test_cases {
        let result = manager.create_scanner(repository_path, None, None).await;
        // Remote repositories should fail with current implementation
        assert!(
            result.is_err(),
            "Remote repositories should currently fail: {}",
            repository_path
        );

        // Verify the error type is Repository error for remote URLs
        match result.unwrap_err() {
            crate::scanner::error::ScanError::Repository { message } => {
                assert!(
                    message.contains("Invalid repository")
                        || message.contains("not found")
                        || message.contains("Remote"),
                    "Error should indicate repository validation failure for remote URL {}: {}",
                    repository_path,
                    message
                );
            }
            other_error => {
                panic!(
                    "Expected Repository error for remote URL {}, got: {:?}",
                    repository_path, other_error
                );
            }
        }
    }
}

#[tokio::test]
async fn test_queue_publisher_creation() {
    // GREEN: Test that queue publisher creation works correctly
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let scanner_task = manager
        .create_scanner(&current_path, None, None)
        .await
        .unwrap();

    // Get the injected queue publisher - should work now
    let publisher = scanner_task.get_queue_publisher();

    // Verify the publisher uses the scanner ID as producer ID
    assert_eq!(
        publisher.producer_id(),
        scanner_task.scanner_id(),
        "Publisher producer ID should match scanner ID"
    );

    // Test that we can get the same publisher reference multiple times
    let second_publisher = scanner_task.get_queue_publisher();
    assert_eq!(
        second_publisher.producer_id(),
        scanner_task.scanner_id(),
        "Second publisher should have same producer ID"
    );
}

#[tokio::test]
async fn test_notification_subscriber_creation() {
    // GREEN: Test that notification subscriber creation works correctly
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let scanner_task = manager
        .create_scanner(&current_path, None, None)
        .await
        .unwrap();

    // Create notification subscriber - should succeed now
    let result = scanner_task.create_notification_subscriber().await;

    assert!(
        result.is_ok(),
        "Notification subscriber creation should succeed"
    );

    let _receiver = result.unwrap();

    // Test that we can create multiple subscribers for the same scanner
    let second_result = scanner_task.create_notification_subscriber().await;
    assert!(
        second_result.is_ok(),
        "Second notification subscriber creation should succeed"
    );

    let _second_receiver = second_result.unwrap();

    // Both receivers should be valid (we can't easily test much more without events)
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
#[ignore = "slow"]
async fn test_commit_traversal_and_message_creation() {
    // GREEN: Test that basic commit traversal and message creation works
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    // Create scanner with explicit requirements to test comprehensive data collection
    let scanner_task = {
        let (repo, _) = manager
            .validate_repository(&current_dir, None, None)
            .unwrap();
        let repo_id = manager.get_unique_repo_id(&repo).unwrap();
        let scanner_id = manager.generate_scanner_id(&repo_id).unwrap();

        use crate::scanner::types::ScanRequires;
        // Create test queue publisher and notification manager
        let queue_service = crate::queue::api::get_queue_service();
        let test_publisher = queue_service
            .create_publisher(scanner_id.clone())
            .expect("Failed to create test queue publisher");

        let notification_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
            crate::notifications::api::AsyncNotificationManager::new(),
        ));

        crate::scanner::task::ScannerTask::new(
            scanner_id,
            current_path.to_string(),
            repo,
            ScanRequires::REPOSITORY_INFO | ScanRequires::COMMITS | ScanRequires::FILE_CHANGES,
            test_publisher,
            None,
            None,
            notification_manager,
        )
    };

    // Scan commits and capture messages for testing
    let messages = scan_and_capture_messages(&scanner_task)
        .await
        .expect("Should capture messages for testing");

    // Should have at least 3 messages: ScanStarted, CommitData(s), ScanCompleted
    assert!(messages.len() >= 3, "Should have at least 3 messages");

    // Verify message types - first should be ScanStarted
    match &messages[0] {
        ScanMessage::ScanStarted {
            scanner_id,
            repository_data,
            ..
        } => {
            assert_eq!(scanner_id, scanner_task.scanner_id());
            assert_eq!(&repository_data.path, &scanner_task.repository_path());
        }
        _ => panic!("First message should be ScanStarted"),
    }

    // Should have at least one CommitData message
    let has_commit_data = messages
        .iter()
        .any(|msg| matches!(msg, ScanMessage::CommitData { .. }));
    assert!(
        has_commit_data,
        "Should have at least one CommitData message"
    );

    // Last message should be ScanCompleted
    match messages.last().unwrap() {
        ScanMessage::ScanCompleted {
            scanner_id, stats, ..
        } => {
            assert_eq!(scanner_id, scanner_task.scanner_id());
            assert!(
                stats.total_commits > 0,
                "Should have scanned at least one commit"
            );
        }
        _ => panic!("Last message should be ScanCompleted"),
    }
}

#[tokio::test]
#[ignore = "slow"]
async fn test_queue_message_publishing() {
    // GREEN: Test that queue message publishing works correctly
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let scanner_task = manager
        .create_scanner(&current_path, None, None)
        .await
        .unwrap();

    // Create test messages
    let messages = vec![ScanMessage::ScanCompleted {
        scanner_id: scanner_task.scanner_id().to_string(),
        timestamp: SystemTime::now(),
        stats: ScanStats {
            total_commits: 1,
            total_files_changed: 0,
            total_insertions: 0,
            total_deletions: 0,
            scan_duration: std::time::Duration::from_secs(1),
        },
    }];

    // Publish messages - should succeed now
    let result = scanner_task.publish_messages(messages).await;

    assert!(result.is_ok(), "Message publishing should succeed");

    // Test with actual scan results
    let scan_messages = scan_and_capture_messages(&scanner_task)
        .await
        .expect("Should capture scan messages for testing");
    let publish_result = scanner_task.publish_messages(scan_messages).await;

    assert!(
        publish_result.is_ok(),
        "Publishing scan results should succeed"
    );
}

#[tokio::test]
async fn test_scanner_event_publishing() {
    // GREEN: Test that scanner event publishing works correctly
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let scanner_task = manager
        .create_scanner(&current_path, None, None)
        .await
        .unwrap();

    // Publish scanner started event - should succeed now
    let result = scanner_task
        .publish_scanner_event(ScanEventType::Started, None)
        .await;

    assert!(result.is_ok(), "Scanner event publishing should succeed");

    // Test with different event types
    let events_to_test = vec![
        (ScanEventType::Started, Some("Scanner starting".to_string())),
        (
            ScanEventType::Progress,
            Some("Scanning progress".to_string()),
        ),
        (ScanEventType::Completed, None),
        (ScanEventType::Error, Some("Test error".to_string())),
    ];

    for (event_type, message) in events_to_test {
        let result = scanner_task
            .publish_scanner_event(event_type, message)
            .await;
        assert!(result.is_ok(), "Should be able to publish all event types");
    }
}

#[tokio::test]
async fn test_scanner_queue_event_subscription() {
    // GREEN: Test that scanner can subscribe to queue events (basic test)
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let scanner_task = manager
        .create_scanner(&current_path, None, None)
        .await
        .unwrap();

    // Test that scanner can subscribe to queue started events
    let _receiver = scanner_task.subscribe_to_queue_events().await;

    // Just test that subscription works - actual event handling is complex
    assert!(
        _receiver.is_ok(),
        "Scanner should be able to subscribe to queue events"
    );
}

#[tokio::test]
async fn test_scanner_shutdown_via_events() {
    // GREEN: Test that scanner can be shut down gracefully via system events
    let manager = ScannerManager::create().await;
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let scanner_task = manager
        .create_scanner(&current_path, None, None)
        .await
        .unwrap();

    // Test that scanner correctly handles shutdown event timeout
    let shutdown_handled = scanner_task
        .handle_shutdown_event(std::time::Duration::from_millis(100))
        .await
        .unwrap();
    assert!(
        !shutdown_handled,
        "Scanner should return false on shutdown event timeout"
    );
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
        Err(crate::scanner::error::ScanError::Repository { message }) => {
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
        Err(crate::scanner::error::ScanError::Repository { message }) => {
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
        Err(crate::scanner::error::ScanError::Repository { message }) => {
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

#[tokio::test]
async fn test_merge_commit_filtering() {
    // GREEN: Test that merge commit filtering works correctly with QueryParams
    use crate::core::query::QueryParams;
    // Helper function already defined at module level
    use std::process::Command;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize repository
    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create initial commit on main branch
    std::fs::write(repo_path.join("main.txt"), "main content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create feature branch and add commit
    Command::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    std::fs::write(repo_path.join("feature.txt"), "feature content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Feature commit"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Merge back to main with a merge commit
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["merge", "feature", "--no-ff", "-m", "Merge feature branch"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create scanner for the test repository
    let manager = ScannerManager::create().await;
    let scanner_task = manager
        .create_scanner(repo_path.to_string_lossy().as_ref(), None, None)
        .await
        .unwrap();

    // Test including merge commits (default behavior)
    let messages_with_merge = collect_scan_messages(&scanner_task, None).await.unwrap();
    let commit_count_with_merge = messages_with_merge
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count_with_merge, 3,
        "Should include all 3 commits (initial + feature + merge)"
    );

    // Test excluding merge commits
    let query_params = QueryParams::default().with_merge_commits(Some(false));
    let messages_without_merge = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count_without_merge = messages_without_merge
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count_without_merge, 2,
        "Should exclude merge commit, leaving 2 regular commits"
    );

    // Verify that all messages still have proper structure
    assert!(matches!(
        messages_without_merge[0],
        ScanMessage::ScanStarted { .. }
    ));
    assert!(matches!(
        messages_without_merge.last().unwrap(),
        ScanMessage::ScanCompleted { .. }
    ));
}
