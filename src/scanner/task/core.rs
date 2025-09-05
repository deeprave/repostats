//! Scanner Task Core Implementation
//!
//! Core ScannerTask struct and basic methods including constructors and accessors.

use crate::core::query::QueryParams;
use crate::queue::api::QueuePublisher;
use crate::scanner::types::ScanRequires;
use std::sync::{Arc, Mutex};

/// Individual scanner task for a specific repository
#[derive(Debug)]
pub struct ScannerTask {
    /// Unique scanner ID (scan-<16_char_sha256>)
    /// Uses first 16 characters of SHA256 hash for strong collision resistance while maintaining
    /// sufficient uniqueness for typical repository scanning workflows.
    /// Collision probability: ~1 in 16^16 (2^64) - extremely low for practical use cases.
    scanner_id: String,
    /// Repository path (local or remote URL - normalized)
    repository_path: String,
    /// Git repository instance
    repository: gix::Repository,
    /// Flag indicating if this is a remote repository
    is_remote: bool,
    /// Combined requirements from all active plugins
    requirements: ScanRequires,
    /// Query parameters for filtering and customizing the scan
    query_params: Option<QueryParams>,
    /// Injected checkout manager for FILE_CONTENT operations (None if no checkout required)
    checkout_manager: Option<Arc<Mutex<crate::scanner::checkout::manager::CheckoutManager>>>,
    /// Injected queue publisher (shared by scanner manager)
    pub(crate) queue_publisher: QueuePublisher,
}

impl ScannerTask {
    /// Create a new ScannerTask with dependency injection
    pub fn new(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
        requirements: ScanRequires,
        queue_publisher: QueuePublisher,
        query_params: Option<QueryParams>,
        checkout_manager: Option<Arc<Mutex<crate::scanner::checkout::manager::CheckoutManager>>>,
    ) -> Self {
        let is_remote = Self::is_remote_path(&repository_path);

        Self {
            scanner_id,
            repository_path,
            repository,
            is_remote,
            requirements,
            query_params,
            checkout_manager,
            queue_publisher,
        }
    }

    /// Create a simple builder for backward compatibility with tests
    /// This is deprecated in favor of direct constructor injection
    #[cfg(test)]
    pub fn builder(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
    ) -> TestScannerTaskBuilder {
        TestScannerTaskBuilder {
            scanner_id,
            repository_path,
            repository,
            requirements: ScanRequires::NONE,
        }
    }
}

/// Simplified builder for test compatibility only
/// Production code should use ScannerTask::new() directly
#[cfg(test)]
pub struct TestScannerTaskBuilder {
    scanner_id: String,
    repository_path: String,
    repository: gix::Repository,
    requirements: ScanRequires,
}

#[cfg(test)]
impl TestScannerTaskBuilder {
    pub fn with_requirements(mut self, requirements: ScanRequires) -> Self {
        self.requirements = requirements;
        self
    }

    pub fn build(self) -> ScannerTask {
        // Create test queue publisher automatically
        let queue_service = crate::queue::api::get_queue_service();
        let test_publisher = queue_service
            .create_publisher(self.scanner_id.clone())
            .expect("Failed to create test queue publisher");

        ScannerTask::new(
            self.scanner_id,
            self.repository_path,
            self.repository,
            self.requirements,
            test_publisher,
            None,
            None,
        )
    }
}

impl ScannerTask {
    /// Create a new ScannerTask with repository (for test compatibility)
    #[cfg(test)]
    pub fn new_with_repository(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
    ) -> Self {
        // Create a test queue publisher for test scenarios
        let queue_service = crate::queue::api::get_queue_service();
        let test_publisher = queue_service
            .create_publisher(scanner_id.clone())
            .expect("Failed to create test queue publisher");

        Self::new(
            scanner_id,
            repository_path,
            repository,
            ScanRequires::NONE,
            test_publisher,
            None,
            None,
        )
    }

    /// Determine if a repository path represents a remote repository
    fn is_remote_path(path: &str) -> bool {
        // Check for explicit URL schemes (git, http, ssh, etc.)
        if path.contains("://") {
            return true;
        }

        // Check for SSH-style remote paths like git@hostname:path
        if path.contains('@') && path.contains(':') && !path.starts_with('/') {
            // Additional check to avoid false positives like C:\ on Windows
            if path.len() > 3
                && path.chars().nth(1) == Some(':')
                && path.chars().nth(2) == Some('\\')
            {
                return false; // Windows path like C:\path
            }
            if path.len() > 2
                && path.chars().nth(1) == Some(':')
                && path.chars().nth(2) != Some('@')
            {
                return false; // Windows path like C:path or drive letter
            }
            return true;
        }

        // All other paths are considered local
        false
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

    /// Get the requirements for this scanner task
    pub fn requirements(&self) -> ScanRequires {
        self.requirements
    }

    /// Check if this is a remote repository
    pub fn is_remote(&self) -> bool {
        self.is_remote
    }

    /// Get reference to the checkout manager if available
    pub fn checkout_manager(
        &self,
    ) -> Option<&Arc<Mutex<crate::scanner::checkout::manager::CheckoutManager>>> {
        self.checkout_manager.as_ref()
    }

    /// Get reference to the query parameters if available
    pub fn query_params(&self) -> Option<&QueryParams> {
        self.query_params.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_remote_path() {
        // URL schemes should be remote
        assert!(ScannerTask::is_remote_path(
            "https://github.com/user/repo.git"
        ));
        assert!(ScannerTask::is_remote_path(
            "git://github.com/user/repo.git"
        ));
        assert!(ScannerTask::is_remote_path(
            "ssh://git@github.com/user/repo.git"
        ));

        // SSH-style paths should be remote
        assert!(ScannerTask::is_remote_path("git@github.com:user/repo.git"));
        assert!(ScannerTask::is_remote_path("user@hostname:path/to/repo"));

        // Windows paths should be local
        assert!(!ScannerTask::is_remote_path("C:\\Users\\user\\repo"));
        assert!(!ScannerTask::is_remote_path("C:/Users/user/repo"));
        assert!(!ScannerTask::is_remote_path("D:\\path\\to\\repo"));
        assert!(!ScannerTask::is_remote_path("C:path"));

        // Unix paths should be local
        assert!(!ScannerTask::is_remote_path("/home/user/repo"));
        assert!(!ScannerTask::is_remote_path("/var/lib/repo"));
        assert!(!ScannerTask::is_remote_path("./relative/path"));
        assert!(!ScannerTask::is_remote_path("../relative/path"));
        assert!(!ScannerTask::is_remote_path("relative/path"));

        // Edge cases
        assert!(!ScannerTask::is_remote_path(""));
        assert!(!ScannerTask::is_remote_path("simple-name"));
    }
}
