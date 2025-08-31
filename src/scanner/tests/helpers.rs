//! Test helper functions for scanner tests
//!
//! This module provides reusable utilities for testing scanner functionality,
//! particularly for collecting and validating scan messages.

use crate::core::query::QueryParams;
use crate::scanner::error::ScanResult;
use crate::scanner::task::ScannerTask;
use crate::scanner::types::ScanMessage;

/// Test helper to collect scan messages into a Vec using the streaming callback API
///
/// This helper eliminates the duplicated pattern of message collection that appears
/// in many test cases, making tests more maintainable and consistent.
pub async fn collect_scan_messages(
    scanner_task: &ScannerTask,
    query_params: Option<&QueryParams>,
) -> ScanResult<Vec<ScanMessage>> {
    let mut messages = Vec::new();
    scanner_task
        .scan_commits_with_query(query_params, |msg| {
            messages.push(msg);
            Ok(())
        })
        .await?;
    Ok(messages)
}

/// Test helper to collect just commit data messages, filtering out other message types
pub async fn collect_commit_messages(
    scanner_task: &ScannerTask,
    query_params: Option<&QueryParams>,
) -> ScanResult<Vec<ScanMessage>> {
    let messages = collect_scan_messages(scanner_task, query_params).await?;
    Ok(messages
        .into_iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .collect())
}

/// Test helper to count commit data messages without collecting all messages
pub async fn count_commit_messages(
    scanner_task: &ScannerTask,
    query_params: Option<&QueryParams>,
) -> ScanResult<usize> {
    let messages = collect_scan_messages(scanner_task, query_params).await?;
    Ok(messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count())
}

/// Test helper to validate standard message sequence and scanner_id consistency
pub fn assert_scan_message_sequence(messages: &[ScanMessage], expected_scanner_id: &str) {
    assert!(messages.len() >= 4, "Should have at least 4 messages: RepositoryData, ScanStarted, CommitData(s), ScanCompleted");

    // Verify message sequence
    assert!(
        matches!(messages[0], ScanMessage::RepositoryData { .. }),
        "First message should be RepositoryData"
    );
    assert!(
        matches!(messages[1], ScanMessage::ScanStarted { .. }),
        "Second message should be ScanStarted"
    );
    assert!(
        matches!(messages.last().unwrap(), ScanMessage::ScanCompleted { .. }),
        "Last message should be ScanCompleted"
    );

    // Verify all messages have correct scanner_id
    for message in messages {
        let id = match message {
            ScanMessage::RepositoryData { scanner_id, .. } => scanner_id,
            ScanMessage::ScanStarted { scanner_id, .. } => scanner_id,
            ScanMessage::CommitData { scanner_id, .. } => scanner_id,
            ScanMessage::ScanCompleted { scanner_id, .. } => scanner_id,
            ScanMessage::FileChange { scanner_id, .. } => scanner_id,
            ScanMessage::ScanError { scanner_id, .. } => scanner_id,
        };
        assert_eq!(
            id, expected_scanner_id,
            "All messages should have consistent scanner_id"
        );
    }
}
