//! Scanner task-related integration tests
//!
//! Tests for scanner task initialization, queue operations, and notification handling.

use repostats::scanner::api::{ScannerManager, ScannerTask, ScanRequires};

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
            repostats::scanner::api::ScanError::Repository { message } => {
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
async fn test_comprehensive_scanner_task_creation() {
    // Test creating scanner task with comprehensive requirements
    let current_dir = std::env::current_dir().unwrap();
    let current_path = current_dir.to_string_lossy();

    let manager = repostats::scanner::api::ScannerManager::create().await;

    let scanner_task = {
        let (repo, _) = manager
            .validate_repository(&current_dir, None, None)
            .unwrap();
        let repo_id = manager.get_unique_repo_id(&repo).unwrap();
        let scanner_id = manager.generate_scanner_id(&repo_id).unwrap();

        // Create test queue publisher and notification manager
        let queue_service = repostats::queue::api::get_queue_service();
        let test_publisher = queue_service
            .create_publisher(scanner_id.clone())
            .expect("Failed to create test queue publisher");

        let notification_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
            repostats::notifications::api::AsyncNotificationManager::new(),
        ));

        ScannerTask::new(
            scanner_id,
            current_path.to_string(),
            repo,
            ScanRequires::COMMITS | ScanRequires::FILE_CHANGES,
            test_publisher,
            None,
            None,
            notification_manager,
        )
    };

    // Verify scanner task properties
    assert_eq!(scanner_task.scanner_id().len(), 16);
    assert!(!scanner_task.repository_path().is_empty());

    // Test that repository is accessible
    let repo = scanner_task.repository();
    assert!(!repo.git_dir().as_os_str().is_empty());
}
