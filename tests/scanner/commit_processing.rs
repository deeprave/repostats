//! Commit processing integration tests
//!
//! Tests for commit traversal, merge commit filtering, and related functionality.

use crate::common;
use repostats::core::query::QueryParams;
use repostats::scanner::api::{ScanMessage, ScannerManager};
use serial_test::serial;
use std::process::Command;
use tempfile::TempDir;

#[tokio::test]
#[serial]
async fn test_merge_commit_filtering() {
    // GREEN: Test that merge commit filtering works correctly with QueryParams
    use common::scanner_helpers::collect_scan_messages;
    use repostats::core::query::QueryParams;
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

#[tokio::test]
#[serial]
async fn test_commit_traversal_with_limits() {
    // Test commit traversal with various limit configurations
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

    // Create multiple commits
    for i in 1..=5 {
        std::fs::write(
            repo_path.join(&format!("file{}.txt", i)),
            format!("content {}", i),
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("Commit {}", i)])
            .current_dir(&repo_path)
            .output()
            .unwrap();
    }

    let manager = ScannerManager::create().await;
    let scanner_task = manager
        .create_scanner(repo_path.to_string_lossy().as_ref(), None, None)
        .await
        .unwrap();

    // Test max_commits limit
    let query_params = QueryParams::new().with_max_commits(Some(2));
    let messages = common::scanner_helpers::collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 2,
        "Should return exactly 2 commits when max_commits is 2"
    );

    // Test no limit (should get all commits)
    let query_params_unlimited = QueryParams::new();
    let all_messages = common::scanner_helpers::collect_scan_messages(&scanner_task, Some(&query_params_unlimited))
        .await
        .unwrap();
    let all_commit_count = all_messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        all_commit_count, 5,
        "Should return all 5 commits with no limit"
    );

    // Test zero limit
    let query_params_zero = QueryParams::new().with_max_commits(Some(0));
    let zero_messages = common::scanner_helpers::collect_scan_messages(&scanner_task, Some(&query_params_zero))
        .await
        .unwrap();
    let zero_commit_count = zero_messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        zero_commit_count, 0,
        "Should return 0 commits when max_commits is 0"
    );
}

#[tokio::test]
#[serial]
async fn test_commit_traversal_ordering() {
    // Test that commits are returned in proper chronological order
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

    // Create commits with identifiable messages
    let commit_messages = vec![
        "First commit",
        "Second commit",
        "Third commit",
        "Fourth commit",
    ];

    for (i, message) in commit_messages.iter().enumerate() {
        std::fs::write(
            repo_path.join(&format!("file{}.txt", i + 1)),
            format!("content {}", i + 1),
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Small delay to ensure commit times are different
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let manager = ScannerManager::create().await;
    let scanner_task = manager
        .create_scanner(repo_path.to_string_lossy().as_ref(), None, None)
        .await
        .unwrap();

    // Test commit ordering (should be newest first)
    let messages = common::scanner_helpers::collect_scan_messages(&scanner_task, None).await.unwrap();
    let commit_data: Vec<_> = messages
        .iter()
        .filter_map(|m| match m {
            ScanMessage::CommitData { commit_info, .. } => Some(commit_info),
            _ => None,
        })
        .collect();

    assert!(commit_data.len() >= 4, "Should have at least 4 commits");

    // Verify commits are in reverse chronological order (newest first)
    // The first commit returned should be "Fourth commit" (the most recent)
    assert!(
        commit_data[0].message.contains("Fourth commit"),
        "First returned commit should be the most recent"
    );
    assert!(
        commit_data[commit_data.len() - 1]
            .message
            .contains("First commit"),
        "Last returned commit should be the oldest"
    );

    // Test with limit to verify ordering is preserved
    let query_params = QueryParams::new().with_max_commits(Some(2));
    let limited_messages = common::scanner_helpers::collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let limited_commits: Vec<_> = limited_messages
        .iter()
        .filter_map(|m| match m {
            ScanMessage::CommitData { commit_info, .. } => Some(commit_info),
            _ => None,
        })
        .collect();

    assert_eq!(limited_commits.len(), 2, "Should return exactly 2 commits");
    assert!(
        limited_commits[0].message.contains("Fourth commit"),
        "First commit should be the most recent"
    );
    assert!(
        limited_commits[1].message.contains("Third commit"),
        "Second commit should be the second most recent"
    );
}

#[tokio::test]
#[serial]
async fn test_empty_repository_commit_traversal() {
    // Test commit traversal behavior on empty repositories
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize empty repository
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

    // Don't create any commits - leave repository empty
    let manager = ScannerManager::create().await;
    let scanner_task = manager
        .create_scanner(repo_path.to_string_lossy().as_ref(), None, None)
        .await
        .unwrap();

    // Test empty repository traversal
    let messages = common::scanner_helpers::collect_scan_messages(&scanner_task, None).await.unwrap();

    // Should have ScanStarted and ScanCompleted, but no CommitData messages
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, ScanMessage::ScanStarted { .. })),
        "Should have ScanStarted message"
    );
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, ScanMessage::ScanCompleted { .. })),
        "Should have ScanCompleted message"
    );

    // Should have no commit data messages
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(commit_count, 0, "Empty repository should have no commits");

    // Test with query parameters - should still work
    let query_params = QueryParams::new().with_max_commits(Some(10));
    let limited_messages = common::scanner_helpers::collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();

    let limited_commit_count = limited_messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        limited_commit_count, 0,
        "Empty repository with limit should still have no commits"
    );
}
