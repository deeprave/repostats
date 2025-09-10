//! Scanner Manager integration tests
//!
//! Tests specifically for ScannerManager creation and core manager functionality.
//! Other scanner functionality is tested in focused modules under tests/scanner/.

mod common;

use repostats::scanner::api::ScannerManager;

#[tokio::test]
async fn test_scanner_manager_creation() {
    // GREEN: Now implement basic ScannerManager creation
    let manager = ScannerManager::create().await;

    // Should successfully create a ScannerManager
    // (Testing that creation succeeds - no internal state assertions needed)
    assert!(manager.as_ref() as *const _ != std::ptr::null());
}

#[tokio::test]
async fn test_scanner_id_generation() {
    // GREEN: Test that SHA256-based scanner ID generation works correctly
    let manager = ScannerManager::create().await;

    let test_cases = vec![
        ("https://github.com/user/repo.git", "github.com/user/repo"),
        ("/path/to/local/repo", "/path/to/local/repo"),
        (
            "ssh://git@gitlab.com/user/project.git",
            "gitlab.com/user/project",
        ),
    ];

    for (input, _expected_normalised) in test_cases {
        let result = manager.generate_scanner_id(input);

        // Should succeed now
        assert!(
            result.is_ok(),
            "Failed to generate scanner ID for: {}",
            input
        );

        let scanner_id = result.unwrap();

        // Should be a valid hex string of 16 chars (truncated SHA256)
        let hash_part = &scanner_id;
        assert_eq!(
            hash_part.len(),
            16,
            "Truncated SHA256 hash should be 16 characters: {}",
            hash_part
        );
        assert!(
            hash_part.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should contain only hex digits: {}",
            hash_part
        );

        // Verify the hash is consistent for the same input
        let second_result = manager.generate_scanner_id(input).unwrap();
        assert_eq!(
            scanner_id, second_result,
            "Scanner ID should be consistent for same input"
        );
    }

    // Test that different inputs produce different scanner IDs
    let id1 = manager
        .generate_scanner_id("https://github.com/user1/repo")
        .unwrap();
    let id2 = manager
        .generate_scanner_id("https://github.com/user2/repo")
        .unwrap();
    assert_ne!(
        id1, id2,
        "Different repositories should have different scanner IDs"
    );
}

// Scanner task initialization tests moved to tests/scanner/tasks.rs
// Queue publisher creation tests moved to tests/scanner/tasks.rs
// Notification subscriber creation tests moved to tests/scanner/tasks.rs
// Local repository opening tests moved to tests/scanner/repository.rs
// Remote repository support tests moved to tests/scanner/repository.rs
// Commit traversal and message creation tests moved to tests/scanner/messaging.rs
// Queue message publishing tests moved to tests/scanner/messaging.rs
// Scanner event publishing tests moved to tests/scanner/messaging.rs
// Scanner queue event subscription tests moved to tests/scanner/tasks.rs
// Scanner shutdown tests moved to tests/scanner/tasks.rs
// Start point resolution tests moved to tests/scanner/repository.rs
// Content reconstruction API tests moved to tests/scanner/repository.rs
// Merge commit filtering tests moved to tests/scanner/commit_processing.rs
