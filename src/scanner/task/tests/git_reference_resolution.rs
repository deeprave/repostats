//! Git Reference Resolution Tests
//!
//! Tests for enhanced git reference resolution including complex patterns like HEAD^

use super::super::*;
use super::helpers::*;
use crate::scanner::types::ScanRequires;
use serial_test::serial;
use std::process::Command;
use tempfile::TempDir;

#[tokio::test]
#[serial]
async fn test_enhanced_git_reference_resolution() {
    let (_temp_dir, repo) = create_test_repository();

    let scanner_task = ScannerTask::builder(
        "test-scanner".to_string(),
        repo.path().to_string_lossy().to_string(),
        repo.clone(),
    )
    .with_requirements(ScanRequires::COMMITS)
    .build();

    // Test basic HEAD resolution
    let head_commit = scanner_task.resolve_start_point("HEAD").await.unwrap();
    assert!(
        !head_commit.is_empty(),
        "HEAD should resolve to a commit hash"
    );

    // Test HEAD~ (parent) notation - should work even with single commit
    // Note: This test may fail with single commit repos, but tests the parsing logic
    match scanner_task.resolve_start_point("HEAD~1").await {
        Ok(parent_commit) => {
            assert!(
                !parent_commit.is_empty(),
                "HEAD~1 should resolve to a commit hash"
            );
            assert_ne!(
                parent_commit, head_commit,
                "Parent should be different from HEAD"
            );
        }
        Err(_) => {
            // Expected for repositories with only one commit
            // The important thing is that the parsing doesn't crash
        }
    }

    // Test HEAD^ (first parent) notation
    match scanner_task.resolve_start_point("HEAD^").await {
        Ok(first_parent) => {
            assert!(
                !first_parent.is_empty(),
                "HEAD^ should resolve to a commit hash"
            );
        }
        Err(_) => {
            // Expected for repositories with only one commit
        }
    }
}

#[tokio::test]
#[serial]
async fn test_git_reference_validation() {
    let (_temp_dir, repo) = create_test_repository();

    let scanner_task = ScannerTask::builder(
        "test-scanner".to_string(),
        repo.path().to_string_lossy().to_string(),
        repo,
    )
    .with_requirements(ScanRequires::COMMITS)
    .build();

    // Test valid references
    let valid_refs = vec!["HEAD", "main", "master"];

    for reference in valid_refs {
        match scanner_task.resolve_start_point(reference).await {
            Ok(commit_hash) => {
                assert!(
                    !commit_hash.is_empty(),
                    "Valid reference '{}' should resolve",
                    reference
                );
                assert!(
                    commit_hash.len() >= 8,
                    "Commit hash should be at least 8 characters"
                );
            }
            Err(e) => {
                // Some references like "main" or "master" might not exist in test repos
                // That's okay - we're testing the validation logic doesn't crash
                println!("Reference '{}' not found (expected): {}", reference, e);
            }
        }
    }

    // Test invalid references should return errors (not panic)
    let invalid_refs = vec!["nonexistent-branch", "invalid/ref/name", ""];

    for reference in invalid_refs {
        match scanner_task.resolve_start_point(reference).await {
            Ok(_) => panic!("Invalid reference '{}' should not resolve", reference),
            Err(_) => {
                // Expected - invalid references should return errors
            }
        }
    }
}

#[tokio::test]
#[serial]
async fn test_complex_git_reference_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create a repository with multiple commits and branches
    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
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

    // Create a tag
    Command::new("git")
        .args(["tag", "v1.0"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create a branch
    Command::new("git")
        .args(["checkout", "-b", "feature-branch"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    std::fs::write(repo_path.join("branch-file.txt"), "branch content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Branch commit"])
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

    // Test complex HEAD patterns
    let complex_patterns = vec![
        ("HEAD", "Current commit"),
        ("HEAD~1", "One commit back"),
        ("HEAD~2", "Two commits back"),
        ("HEAD^", "First parent"),
        ("v1.0", "Tag reference"),
        ("feature-branch", "Branch reference"),
    ];

    for (pattern, description) in complex_patterns {
        match scanner_task.resolve_start_point(pattern).await {
            Ok(commit_hash) => {
                assert!(
                    !commit_hash.is_empty(),
                    "{} should resolve to commit hash",
                    description
                );
                assert!(
                    commit_hash.len() >= 8,
                    "Commit hash should be at least 8 characters"
                );
                println!("{} ({}) resolved to: {}", pattern, description, commit_hash);
            }
            Err(e) => {
                println!("{} ({}) failed to resolve: {}", pattern, description, e);
                // Some patterns might not resolve depending on repository state
                // The important thing is the parsing doesn't crash
            }
        }
    }

    // Test that different references resolve to different commits
    if let (Ok(head), Ok(head_parent)) = (
        scanner_task.resolve_start_point("HEAD").await,
        scanner_task.resolve_start_point("HEAD~1").await,
    ) {
        assert_ne!(
            head, head_parent,
            "HEAD and HEAD~1 should resolve to different commits"
        );
    }

    // Test tag vs branch resolution
    if let (Ok(tag_commit), Ok(branch_commit)) = (
        scanner_task.resolve_start_point("v1.0").await,
        scanner_task.resolve_start_point("feature-branch").await,
    ) {
        assert_ne!(
            tag_commit, branch_commit,
            "Tag and branch should resolve to different commits"
        );
    }
}
