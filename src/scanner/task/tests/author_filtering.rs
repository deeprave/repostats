//! Author Filtering Tests
//!
//! Tests for author pattern matching and filtering functionality

// Removed unused import: super::helpers::*
use super::super::*;
use crate::core::query::QueryParams;
use crate::scanner::tests::helpers::{collect_scan_messages, count_commit_messages};
use crate::scanner::types::{ScanMessage, ScanRequires};
use std::process::Command;
use tempfile::TempDir;

#[tokio::test]
async fn test_commit_traversal_with_author_filtering() {
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

    // Create commits with different authors
    std::fs::write(repo_path.join("file1.txt"), "content1").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "First commit"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Change author for second commit
    Command::new("git")
        .args(["config", "user.name", "Another User"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "another@test.org"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    std::fs::write(repo_path.join("file2.txt"), "content2").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Second commit"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create third commit back to original author
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

    std::fs::write(repo_path.join("file3.txt"), "content3").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Third commit"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder(
        "test-scanner".to_string(),
        repo.path().to_string_lossy().to_string(),
        repo,
    )
    .with_requirements(ScanRequires::COMMITS)
    .build();

    // Test 1: Exact email match
    let query_params = QueryParams::new().with_authors(vec!["test@example.com".to_string()]);
    let commit_count = count_commit_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    assert_eq!(commit_count, 2, "Should find 2 commits by test@example.com");

    // Test 2: Email wildcard - domain pattern
    let query_params = QueryParams::new().with_authors(vec!["*@example.com".to_string()]);
    let commit_count = count_commit_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    assert_eq!(
        commit_count, 2,
        "Should find 2 commits with *@example.com pattern"
    );

    // Test 3: Email wildcard - broader domain pattern
    let query_params = QueryParams::new().with_authors(vec!["*@*.org".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(commit_count, 1, "Should find 1 commit with *@*.org pattern");

    // Test 4: Name wildcard pattern - case insensitive
    let query_params = QueryParams::new().with_authors(vec!["test*".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 2,
        "Should find 2 commits with 'test*' name pattern"
    );

    // Test 5: Name wildcard - partial word match (matches both "Test User" and "Another User")
    let query_params = QueryParams::new().with_authors(vec!["*User".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 3,
        "Should find 3 commits with '*User' name pattern (both Test User and Another User)"
    );

    // Test 6: Case insensitive name matching
    let query_params = QueryParams::new().with_authors(vec!["ANOTHER*".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 1,
        "Should find 1 commit with case-insensitive 'ANOTHER*' pattern"
    );
}

#[tokio::test]
async fn test_complex_wildcard_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create commits with complex names and emails
    let authors = [
        ("David Smith", "david@amazon.com"),
        ("David Johnson", "david.johnson@aws.amazon.com"),
        ("John \"the Coder\" Smith", "john@microsoft.com"),
    ];

    for (i, (name, email)) in authors.iter().enumerate() {
        Command::new("git")
            .args(["config", "user.name", name])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", email])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        std::fs::write(
            repo_path.join(&format!("file{}.txt", i + 1)),
            format!("content{}", i + 1),
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("Commit {}", i + 1)])
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

    // Test 1: Complex email domain pattern - should match aws.amazon.com and amazon.com
    let query_params = QueryParams::new().with_authors(vec!["*@*amazon.com".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 2,
        "Should match *@*amazon.com pattern (aws.amazon.com and amazon.com)"
    );

    // Test 2: Specific domain pattern
    let query_params = QueryParams::new().with_authors(vec!["*@amazon.com".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(commit_count, 1, "Should match exact *@amazon.com pattern");

    // Test 3: Case-insensitive name pattern with complex names
    let query_params = QueryParams::new().with_authors(vec!["david*".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 2,
        "Should match all David variants case-insensitively"
    );

    // Test 4: Pattern with special characters
    let query_params = QueryParams::new().with_authors(vec!["*\"the*\"*".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(commit_count, 1, "Should match name with special characters");
}

#[tokio::test]
async fn test_email_auto_completion_integration() {
    // Test that auto-completion works through the full Git scanning pipeline
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create commits with different email patterns
    let authors = [
        ("User One", "user1@example.com"),
        ("User Two", "user2@example.com"),
        ("User Three", "user@test.org"),
    ];

    for (i, (name, email)) in authors.iter().enumerate() {
        Command::new("git")
            .args(["config", "user.name", name])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", email])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        std::fs::write(
            repo_path.join(&format!("file{}.txt", i + 1)),
            format!("content{}", i + 1),
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("Commit {}", i + 1)])
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

    // Test 1: @example.com should auto-complete to *@example.com (match both users at example.com)
    let query_params = QueryParams::new().with_authors(vec!["@example.com".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 2,
        "Auto-completion '@example.com' → '*@example.com' should match 2 commits"
    );

    // Test 2: user@ should auto-complete to user@* (match user at any domain)
    let query_params = QueryParams::new().with_authors(vec!["user@".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 1,
        "Auto-completion 'user@' → 'user@*' should match 1 commit"
    );

    // Test 3: @ should auto-complete to *@* (match all email addresses)
    let query_params = QueryParams::new().with_authors(vec!["@".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 3,
        "Auto-completion '@' → '*@*' should match all 3 commits"
    );

    // Test 4: Explicit wildcards should still work (no auto-completion needed)
    let query_params = QueryParams::new().with_authors(vec!["*@*.org".to_string()]);
    let messages = collect_scan_messages(&scanner_task, Some(&query_params))
        .await
        .unwrap();
    let commit_count = messages
        .iter()
        .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
        .count();
    assert_eq!(
        commit_count, 1,
        "Explicit pattern '*@*.org' should match 1 commit (no auto-completion)"
    );
}
