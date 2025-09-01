//! Test Helper Functions
//!
//! Shared utilities for scanner task tests

use super::super::*;
// Removed unused imports: QueryParams, collect_scan_messages, count_commit_messages
use crate::scanner::types::ScanRequires;
use std::process::Command;
use tempfile::TempDir;

/// Helper to create a test git repository
pub fn create_test_repo() -> (TempDir, gix::Repository) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();
    // Initialize a bare git repo for testing
    let repo = gix::init_bare(repo_path).expect("Failed to init git repo");
    (temp_dir, repo)
}

/// Helper to create a ScannerTask with specific requirements
pub fn create_test_scanner_task(requirements: ScanRequires) -> (TempDir, ScannerTask) {
    let (_temp_dir, repo) = create_test_repo();
    let repo_path = repo.git_dir().to_string_lossy().to_string();
    let scanner = ScannerTask::builder("test-scanner-123".to_string(), repo_path, repo)
        .with_requirements(requirements)
        .build();
    (_temp_dir, scanner)
}

/// Helper to create a test git repository with commits and history
pub fn create_test_repository() -> (TempDir, gix::Repository) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to init repository");

    // Configure git user
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to set user.name");

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to set user.email");

    // Create initial commit
    std::fs::write(repo_path.join("file1.txt"), "initial content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to add files");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to commit");

    let repo = gix::open(repo_path).expect("Failed to open repository");
    (temp_dir, repo)
}
