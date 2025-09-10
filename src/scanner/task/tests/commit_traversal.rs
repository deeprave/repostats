//! Commit Traversal Tests
//!
//! Tests for basic commit traversal, limits, and reference-based scanning

use super::super::ScannerTask;
use crate::core::query::QueryParams;
use crate::scanner::error::ScanResult;
use crate::scanner::types::{ScanMessage, ScanRequires};
use serial_test::serial;
use std::process::Command;
use tempfile::TempDir;

/// Test helper to collect scan messages into a Vec using the streaming callback API
async fn collect_scan_messages(
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

#[tokio::test]
#[serial]
async fn test_commit_traversal_with_max_commits() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
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

    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder(
        "test-scanner".to_string(),
        repo.path().to_string_lossy().to_string(),
        repo,
    )
    .with_requirements(ScanRequires::COMMITS)
    .build();

    // Test max_commits limit
    let query_params = QueryParams::new().with_max_commits(Some(2));
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
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
    let all_messages = collect_scan_messages(&scanner_task, Some(&query_params_unlimited))
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
    let zero_messages = collect_scan_messages(&scanner_task, Some(&query_params_zero))
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
async fn test_commit_traversal_with_git_ref() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
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

    // Create initial commits on default branch
    for i in 1..=3 {
        std::fs::write(
            repo_path.join(&format!("main{}.txt", i)),
            format!("main content {}", i),
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("Main commit {}", i)])
            .current_dir(&repo_path)
            .output()
            .unwrap();
    }

    // Create a test branch
    Command::new("git")
        .args(["checkout", "-b", "test-branch"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    for i in 1..=2 {
        std::fs::write(
            repo_path.join(&format!("test{}.txt", i)),
            format!("test content {}", i),
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("Test branch commit {}", i)])
            .current_dir(&repo_path)
            .output()
            .unwrap();
    }

    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder(
        "test-scanner".to_string(),
        repo.path().to_string_lossy().to_string(),
        repo,
    )
    .with_requirements(ScanRequires::COMMITS)
    .build();

    // Test scanning specific branch
    let query_params = QueryParams::new().with_git_ref(Some("test-branch".to_string()));
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 5,
        "test-branch should include its commits plus main branch history"
    );

    // Test scanning HEAD (should be current branch)
    let query_params_head = QueryParams::new().with_git_ref(Some("HEAD".to_string()));
    let head_messages = collect_scan_messages(&scanner_task, Some(&query_params_head))
        .await
        .unwrap();
    let head_commit_count = head_messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        head_commit_count, 5,
        "HEAD should return same as test-branch since it's current"
    );

    // Test with main branch reference (need to check if it exists)
    // Note: The default branch name might be 'master' or 'main' depending on Git config
    for branch_name in &["main", "master"] {
        if let Ok(messages) = collect_scan_messages(
            &scanner_task,
            Some(&QueryParams::new().with_git_ref(Some(branch_name.to_string()))),
        )
        .await
        {
            let commit_count = messages
                .iter()
                .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
                .count();
            if commit_count > 0 {
                // Found the main branch
                assert!(
                    commit_count >= 3,
                    "{} branch should have at least 3 commits",
                    branch_name
                );
                break;
            }
        }
    }
}

#[tokio::test]
#[serial]
async fn test_commit_traversal_ordering() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
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

    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder(
        "test-scanner".to_string(),
        repo.path().to_string_lossy().to_string(),
        repo,
    )
    .with_requirements(ScanRequires::COMMITS)
    .build();

    // Test commit ordering (should be newest first)
    let messages = collect_scan_messages(&scanner_task, None).await.unwrap();
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
    let limited_messages = collect_scan_messages(&scanner_task, Some(&query_params))
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
