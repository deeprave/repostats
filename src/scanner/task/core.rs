//! Scanner Task Core Implementation
//!
//! Core ScannerTask struct and basic methods including constructors and accessors.

use gix;

/// Individual scanner task for a specific repository
#[derive(Debug)]
pub struct ScannerTask {
    /// Unique scanner ID (scan-<12_char_sha256>)
    /// Uses first 12 characters of SHA256 hash for readability while maintaining
    /// sufficient uniqueness for typical repository scanning workflows.
    /// Collision probability: ~1 in 16^12 (281 trillion) - acceptable for expected usage scale.
    scanner_id: String,
    /// Repository path (local or remote URL - normalized)
    repository_path: String,
    /// Git repository instance
    repository: gix::Repository,
    /// Flag indicating if this is a remote repository
    is_remote: bool,
}

impl ScannerTask {
    /// Create a new ScannerTask with repository instance
    pub fn new_with_repository(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
    ) -> Self {
        Self {
            scanner_id,
            repository_path: repository_path.clone(),
            repository,
            is_remote: repository_path.contains("://") || !repository_path.starts_with('/'),
        }
    }

    /// Get the scanner ID
    pub fn scanner_id(&self) -> &str {
        &self.scanner_id
    }

    /// Get the repository path
    pub fn repository_path(&self) -> &str {
        &self.repository_path
    }

    /// Get reference to the repository instance
    pub fn repository(&self) -> &gix::Repository {
        &self.repository
    }

    /// Check if this is a remote repository
    pub fn is_remote(&self) -> bool {
        self.is_remote
    }
}
