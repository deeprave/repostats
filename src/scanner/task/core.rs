//! Scanner Task Core Implementation
//!
//! Core ScannerTask struct and basic methods including constructors and accessors.

use crate::core::query::QueryParams;
use crate::scanner::api::ScanRequires;
#[cfg(test)]
use crate::scanner::error::ScanResult;
use gix;

/// Individual scanner task for a specific repository
#[derive(Debug)]
pub struct ScannerTask {
    /// Unique scanner ID (scan-<sha256>)
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

    /// Check if this is a remote repository
    pub fn is_remote(&self) -> bool {
        self.is_remote
    }

    /// Get the scanner requirements
    pub fn requirements(&self) -> ScanRequires {
        self.requirements
    }

    /// Get the query parameters
    pub fn query_params(&self) -> Option<&QueryParams> {
        self.query_params.as_ref()
    }

    // Phase 7: Scanner Filters and Query Parameters (Future Implementation)
    /// Apply scanning filters based on query parameters
    #[cfg(test)]
    pub async fn apply_scan_filters(&self, _query_params: QueryParams) -> ScanResult<()> {
        // Test-only placeholder for future filtering logic
        Ok(())
    }

    // Phase 8: Advanced Git Operations (Future Implementation)
    /// Perform advanced git operations for comprehensive scanning
    #[cfg(test)]
    pub async fn perform_advanced_git_operations(&self) -> ScanResult<Vec<String>> {
        // Test-only placeholder for future git operations
        Ok(vec!["advanced-operation-1".to_string()])
    }

    // Phase 9: Integration Testing and Polish (Future Implementation)
    /// Run integration tests for scanner functionality
    #[cfg(test)]
    pub async fn run_integration_tests(&self) -> ScanResult<bool> {
        // Test-only placeholder for future integration testing
        Ok(true)
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
