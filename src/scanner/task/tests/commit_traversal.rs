//! Commit Traversal Tests
//!
//! Tests for basic commit traversal, limits, and reference-based scanning

use super::super::ScannerTask;
use crate::core::query::QueryParams;
use crate::scanner::task::tests::helpers::{commit_all, init_test_git_repo, run_git};
use crate::scanner::tests::helpers::collect_scan_messages;
use crate::scanner::types::{ScanMessage, ScanRequires};
use serial_test::serial;
use tempfile::TempDir;

#[tokio::test]
#[serial]
async fn test_commit_traversal_with_max_commits() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
    init_test_git_repo(repo_path);

    // Create multiple commits
    for i in 1..=5 {
        std::fs::write(
            repo_path.join(format!("file{}.txt", i)),
            format!("content {}", i),
        )
        .unwrap();
        let message = format!("Commit {}", i);
        commit_all(repo_path, &message);
    }

    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder_for_tests(
        "test-scanner".to_string(),
        repo_path.to_string_lossy().to_string(),
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
    init_test_git_repo(repo_path);

    // Create initial commits on default branch
    for i in 1..=3 {
        std::fs::write(
            repo_path.join(format!("main{}.txt", i)),
            format!("main content {}", i),
        )
        .unwrap();
        let message = format!("Main commit {}", i);
        commit_all(repo_path, &message);
    }

    // Create a test branch
    run_git(repo_path, &["checkout", "-b", "test-branch"]);
    for i in 1..=2 {
        std::fs::write(
            repo_path.join(format!("test{}.txt", i)),
            format!("test content {}", i),
        )
        .unwrap();
        let message = format!("Test branch commit {}", i);
        commit_all(repo_path, &message);
    }

    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder_for_tests(
        "test-scanner".to_string(),
        repo_path.to_string_lossy().to_string(),
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
    init_test_git_repo(repo_path);

    // Create commits with identifiable messages
    let commit_messages = [
        "First commit",
        "Second commit",
        "Third commit",
        "Fourth commit",
    ];

    for (i, message) in commit_messages.iter().enumerate() {
        std::fs::write(
            repo_path.join(format!("file{}.txt", i + 1)),
            format!("content {}", i + 1),
        )
        .unwrap();
        commit_all(repo_path, message);

        // Small delay to ensure commit times are different
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder_for_tests(
        "test-scanner".to_string(),
        repo_path.to_string_lossy().to_string(),
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

#[tokio::test]
#[serial]
async fn test_empty_repository_traversal() {
    // TEST: Verify commit traversal behavior when the repository is empty
    // This should handle gracefully without errors and return appropriate messages

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize empty git repository
    init_test_git_repo(repo_path);

    // Don't create any commits - leave repository empty
    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder_for_tests(
        "test-scanner".to_string(),
        repo_path.to_string_lossy().to_string(),
        repo,
    )
    .with_requirements(ScanRequires::COMMITS)
    .build();

    // Test empty repository traversal
    let messages = collect_scan_messages(&scanner_task, None).await.unwrap();

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
    let limited_messages = collect_scan_messages(&scanner_task, Some(&query_params))
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
