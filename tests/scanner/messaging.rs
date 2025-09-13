//! Scanner messaging-related integration tests
//!
//! Tests for message publishing, event handling, and queue operations.

use crate::common;
use repostats::notifications::api::ScanEventType;
use repostats::scanner::api::{ScanMessage, ScanStats, ScannerManager};
use std::time::SystemTime;

/// Test helper to capture scan messages - alias for backward compatibility
async fn scan_and_capture_messages(scanner_task: &repostats::scanner::api::ScannerTask) -> repostats::scanner::api::ScanResult<Vec<ScanMessage>> {
    use std::cell::RefCell;
    use std::rc::Rc;

    let messages = Rc::new(RefCell::new(Vec::new()));
    let messages_clone = Rc::clone(&messages);

    scanner_task
        .scan_commits_with_query(None, move |msg| {
            messages_clone.borrow_mut().push(msg);
            async { Ok(()) }
        })
        .await?;

    Ok(Rc::try_unwrap(messages).unwrap().into_inner())
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

        // Create test queue publisher and notification manager
        let queue_service = repostats::queue::api::get_queue_service();
        let test_publisher = queue_service
            .create_publisher(scanner_id.clone())
            .expect("Failed to create test queue publisher");

        let notification_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
            repostats::notifications::api::AsyncNotificationManager::new(),
        ));

        repostats::scanner::api::ScannerTask::new(
            scanner_id,
            current_path.to_string(),
            repo,
            repostats::scanner::api::ScanRequires::COMMITS | repostats::scanner::api::ScanRequires::FILE_CHANGES,
            test_publisher,
            None,
            None,
            notification_manager,
        )
    };

    // Scan commits and capture messages for testing
    let messages = common::scanner_helpers::scan_and_capture_messages(&scanner_task)
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
