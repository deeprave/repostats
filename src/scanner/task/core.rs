//! Scanner Task Core Implementation
//!
//! Core ScannerTask struct and basic methods including constructors and accessors.

use crate::core::query::QueryParams;
#[cfg(test)]
use crate::scanner::error::ScanResult;
use crate::scanner::types::ScanRequires;

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
    /// Combined requirements from all active plugins
    requirements: ScanRequires,
    /// Query parameters for filtering and customizing the scan
    query_params: Option<QueryParams>,
}

/// Builder for creating ScannerTask instances with optional parameters
pub struct ScannerTaskBuilder {
    scanner_id: String,
    repository_path: String,
    repository: gix::Repository,
    requirements: ScanRequires,
    query_params: Option<QueryParams>,
}

impl ScannerTaskBuilder {
    /// Create a new builder with required parameters
    pub fn new(scanner_id: String, repository_path: String, repository: gix::Repository) -> Self {
        Self {
            scanner_id,
            repository_path,
            repository,
            requirements: ScanRequires::NONE,
            query_params: None,
        }
    }

    /// Set the requirements for the scanner
    pub fn with_requirements(mut self, requirements: ScanRequires) -> Self {
        self.requirements = requirements;
        self
    }

    /// Set the query parameters for the scanner
    pub fn with_query_params(mut self, query_params: QueryParams) -> Self {
        self.query_params = Some(query_params);
        self
    }

    /// Build the ScannerTask
    pub fn build(self) -> ScannerTask {
        let is_remote = ScannerTask::is_remote_path(&self.repository_path);
        ScannerTask {
            scanner_id: self.scanner_id,
            repository_path: self.repository_path,
            repository: self.repository,
            is_remote,
            requirements: self.requirements,
            query_params: self.query_params,
        }
    }
}

impl ScannerTask {
    /// Create a builder for constructing ScannerTask instances
    pub fn builder(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
    ) -> ScannerTaskBuilder {
        ScannerTaskBuilder::new(scanner_id, repository_path, repository)
    }

    /// Create a new ScannerTask with repository (for test compatibility)
    #[cfg(test)]
    pub fn new_with_repository(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
    ) -> Self {
        Self::builder(scanner_id, repository_path, repository).build()
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
    #[cfg(test)]
    pub fn requirements(&self) -> ScanRequires {
        self.requirements
    }

    /// Check if this is a remote repository
    pub fn is_remote(&self) -> bool {
        self.is_remote
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
