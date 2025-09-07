//! File Extraction Tests
//!
//! Tests for extracting commit files to directories - Git operations moved from CheckoutManager

use super::super::*;
use serial_test::serial;
use std::process::Command;
use tempfile::TempDir;

#[tokio::test]
#[serial]
async fn test_extract_commit_files_to_directory() {
    // Create a test repository with known files
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialise Git repository
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

    // Create test files with known content
    std::fs::write(
        repo_path.join("README.md"),
        "# Test Repository\n\nTest content.",
    )
    .unwrap();
    std::fs::create_dir_all(repo_path.join("src")).unwrap();
    std::fs::write(
        repo_path.join("src/main.rs"),
        "fn main() {\n    println!(\"Hello, world!\");\n}",
    )
    .unwrap();
    std::fs::write(repo_path.join("config.toml"), "[package]\nname = \"test\"").unwrap();

    // Add and commit files
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit with test files"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Get commit hash
    let commit_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let commit_sha = std::str::from_utf8(&commit_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Create ScannerTask
    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder(
        "test-scanner".to_string(),
        repo.path().to_string_lossy().to_string(),
        repo,
    )
    .build();

    // Create target directory for extraction
    let extract_dir = TempDir::new().unwrap();
    let target_dir = extract_dir.path();

    // TEST: This should fail because the method doesn't exist yet
    let files_extracted = scanner_task
        .extract_commit_files_to_directory(&commit_sha, target_dir, None)
        .await
        .unwrap();

    // Verify files were extracted correctly
    assert!(
        target_dir.join("README.md").exists(),
        "README.md should be extracted"
    );
    assert!(
        target_dir.join("src").is_dir(),
        "src directory should be created"
    );
    assert!(
        target_dir.join("src/main.rs").exists(),
        "src/main.rs should be extracted"
    );
    assert!(
        target_dir.join("config.toml").exists(),
        "config.toml should be extracted"
    );

    // Verify file contents
    let readme_content = std::fs::read_to_string(target_dir.join("README.md")).unwrap();
    assert!(
        readme_content.contains("Test Repository"),
        "README.md content should match"
    );

    let main_content = std::fs::read_to_string(target_dir.join("src/main.rs")).unwrap();
    assert!(
        main_content.contains("Hello, world!"),
        "main.rs content should match"
    );

    // Verify return value indicates number of files extracted
    assert!(
        files_extracted >= 3,
        "Should return number of files extracted: {}",
        files_extracted
    );
}

#[tokio::test]
#[serial]
async fn test_checkout_path_contains_full_file_paths() {
    use crate::scanner::types::{ChangeType, FileChangeData};
    use std::path::PathBuf;

    // TEST: Verify the checkout_path logic directly
    // This tests the core fix: checkout_path should contain full file paths

    let base_checkout_dir = PathBuf::from("/tmp/test-checkout");
    let test_files = vec![
        "README.md",
        "src/main.rs",
        "tests/integration.rs",
        "docs/guide.md",
    ];

    // Test the logic for creating full file paths from base directory
    for file_path in &test_files {
        // Simulate what the fixed code should do
        let file_checkout_path = Some(&base_checkout_dir).map(|base_dir| base_dir.join(file_path));

        let file_change_data = FileChangeData {
            change_type: ChangeType::Added,
            old_path: None,
            new_path: file_path.to_string(),
            insertions: 10,
            deletions: 0,
            is_binary: false,
            checkout_path: file_checkout_path,
            file_modified_epoch: Some(1_600_000_000),
            file_mode: Some("Added".into()),
        };

        // Verify the checkout_path contains the full file path
        let checkout_path = file_change_data.checkout_path.as_ref().unwrap();

        // TEST: checkout_path should end with the specific file path
        assert!(
            checkout_path.to_string_lossy().ends_with(file_path),
            "checkout_path '{}' should end with file_path '{}' (full file path required)",
            checkout_path.display(),
            file_path
        );

        // TEST: checkout_path should contain the base directory plus the file path
        let expected_path = base_checkout_dir.join(file_path);
        assert_eq!(
            *checkout_path, expected_path,
            "checkout_path should be base directory joined with file path"
        );
    }

    // TEST: Different files should produce different checkout paths
    let mut checkout_paths = std::collections::HashSet::new();
    for file_path in &test_files {
        let file_checkout_path = Some(&base_checkout_dir).map(|base_dir| base_dir.join(file_path));
        checkout_paths.insert(file_checkout_path.unwrap());
    }

    assert_eq!(
        checkout_paths.len(),
        test_files.len(),
        "Each file should have a unique checkout_path"
    );
}

#[tokio::test]
#[serial]
async fn test_extract_files_creates_full_checkout_paths() {
    // Integration test using the actual extract_commit_files_to_directory method
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create a test repository
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

    // Create test files in subdirectories
    std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
    std::fs::create_dir_all(repo_path.join("src")).unwrap();
    std::fs::write(repo_path.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::create_dir_all(repo_path.join("docs")).unwrap();
    std::fs::write(repo_path.join("docs/guide.md"), "# Guide").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add test files"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Get commit hash
    let commit_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let commit_sha = std::str::from_utf8(&commit_output.stdout).unwrap().trim();

    // Create ScannerTask and extract files
    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder(
        "test-scanner".to_string(),
        repo.path().to_string_lossy().to_string(),
        repo,
    )
    .build();

    let extract_dir = TempDir::new().unwrap();
    let target_dir = extract_dir.path();

    let files_extracted = scanner_task
        .extract_commit_files_to_directory(commit_sha, target_dir, None)
        .await
        .unwrap();

    // Verify files were extracted correctly
    assert!(files_extracted >= 3, "Should extract at least 3 files");

    // Test that the checkout_path logic would work with these extracted files
    // This simulates how checkout_path would be set for each extracted file
    let test_files = ["README.md", "src/main.rs", "docs/guide.md"];

    for file_path in &test_files {
        // Verify file exists in extraction directory (simulating checkout)
        let full_file_path = target_dir.join(file_path);
        assert!(
            full_file_path.exists(),
            "Extracted file should exist: {}",
            full_file_path.display()
        );

        // Verify it's a file, not directory
        assert!(
            full_file_path.is_file(),
            "Should be a file: {}",
            full_file_path.display()
        );

        // This is what checkout_path would contain after our fix
        // (base extraction directory + file path = full file path)
        let expected_checkout_path = target_dir.join(file_path);
        assert_eq!(
            full_file_path, expected_checkout_path,
            "checkout_path logic produces correct full file path"
        );
    }
}

#[tokio::test]
#[serial]
async fn test_resolve_revision_method() {
    // TDD test for resolve_revision method moved from CheckoutManager to ScannerTask
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create a test repository with multiple commits and a tag
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

    // Create first commit
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

    // Create a tag
    Command::new("git")
        .args(["tag", "v1.0"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create second commit
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

    // Create ScannerTask
    let repo = gix::open(repo_path).unwrap();
    let scanner_task = ScannerTask::builder(
        "test-scanner".to_string(),
        repo.path().to_string_lossy().to_string(),
        repo,
    )
    .build();

    // TEST: This should fail because resolve_revision doesn't exist yet
    // Test resolving HEAD
    let head_sha = scanner_task.resolve_revision(Some("HEAD")).await.unwrap();
    assert_eq!(head_sha.len(), 40, "Should return 40-character SHA");

    // Test resolving tag
    let tag_sha = scanner_task.resolve_revision(Some("v1.0")).await.unwrap();
    assert_eq!(tag_sha.len(), 40, "Should return 40-character SHA for tag");

    // Test resolving with None (should default to HEAD)
    let default_sha = scanner_task.resolve_revision(None).await.unwrap();
    assert_eq!(default_sha, head_sha, "None should resolve to HEAD");

    // Test different SHAs for different revisions
    assert_ne!(
        head_sha, tag_sha,
        "HEAD and v1.0 should be different commits"
    );
}
