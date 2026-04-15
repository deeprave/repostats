//! Test Helper Functions
//!
//! Shared utilities for scanner task tests

use super::super::*;
// Removed unused imports: QueryParams, collect_scan_messages, count_commit_messages
use crate::notifications::api::AsyncNotificationManager;
use crate::scanner::types::ScanRequires;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex as TokioMutex;

/// Run a git command in a test repository and assert that it succeeds.
pub fn run_git(repo_path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .expect("Failed to spawn git command");

    assert!(
        output.status.success(),
        "git {} failed\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Initialize a non-bare git repository for scanner tests with signing disabled.
pub fn init_test_git_repo(repo_path: &Path) {
    run_git(repo_path, &["init", "--initial-branch=main"]);
    run_git(repo_path, &["config", "user.name", "Test User"]);
    run_git(repo_path, &["config", "user.email", "test@example.com"]);
    run_git(repo_path, &["config", "commit.gpgsign", "false"]);
}

/// Stage all changes and create an unsigned test commit.
pub fn commit_all(repo_path: &Path, message: &str) {
    run_git(repo_path, &["add", "."]);
    run_git(
        repo_path,
        &["-c", "commit.gpgsign=false", "commit", "-m", message],
    );
}

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

    // Create test queue publisher and notification manager
    let queue_service = crate::queue::api::get_queue_service();
    let test_publisher = queue_service
        .create_publisher("test-scanner-123".to_string())
        .expect("Failed to create test queue publisher");

    let notification_manager = Arc::new(TokioMutex::new(AsyncNotificationManager::new()));

    let scanner = ScannerTask::new(
        "test-scanner-123".to_string(),
        repo_path,
        repo,
        requirements,
        test_publisher,
        None,
        None,
        notification_manager,
    );
    (_temp_dir, scanner)
}

/// Helper to create a test git repository with commits and history
pub fn create_test_repository() -> (TempDir, gix::Repository) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    init_test_git_repo(repo_path);

    // Create initial commit
    std::fs::write(repo_path.join("file1.txt"), "initial content").unwrap();
    commit_all(repo_path, "Initial commit");

    let repo = gix::open(repo_path).expect("Failed to open repository");
    (temp_dir, repo)
}
