//! Test helper functions for scanner unit tests
//!
//! This module provides reusable utilities for testing scanner functionality within unit tests.
//! These helpers are separate from integration test helpers in tests/common/ since unit tests
//! and integration tests run in different contexts.

use crate::core::query::QueryParams;
use crate::scanner::error::ScanResult;
use crate::scanner::task::ScannerTask;
use crate::scanner::types::ScanMessage;

/// Test helper to collect scan messages into a Vec using the streaming callback API
///
/// This helper eliminates the duplicated pattern of message collection that appears
/// in many unit test cases, making tests more maintainable and consistent.
///
/// Uses Rc<RefCell<>> for efficient single-threaded test execution.
pub async fn collect_scan_messages(
    scanner_task: &ScannerTask,
    query_params: Option<&QueryParams>,
) -> ScanResult<Vec<ScanMessage>> {
    use std::cell::RefCell;
    use std::rc::Rc;

    let messages = Rc::new(RefCell::new(Vec::new()));
    let messages_clone = Rc::clone(&messages);

    scanner_task
        .scan_commits_with_query(query_params, move |msg| {
            messages_clone.borrow_mut().push(msg);
            async { Ok(()) }
        })
        .await?;

    Ok(Rc::try_unwrap(messages).unwrap().into_inner())
}

/// Alias for collect_scan_messages for backward compatibility
pub async fn scan_and_capture_messages(scanner_task: &ScannerTask) -> ScanResult<Vec<ScanMessage>> {
    collect_scan_messages(scanner_task, None).await
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
