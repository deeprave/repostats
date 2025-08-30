//! Scanner Task Core Implementation
//!
//! Core ScannerTask struct and basic methods including constructors and accessors.

use crate::core::query::QueryParams;
use crate::scanner::api::ScanRequires;
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

impl ScannerTask {
    /// Create a new ScannerTask with repository instance
    pub fn new_with_repository(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
    ) -> Self {
        Self::new_with_all_options(
            scanner_id,
            repository_path,
            repository,
            ScanRequires::NONE,
            None,
        )
    }

    /// Create a new ScannerTask with repository instance and requirements
    pub fn new_with_requirements(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
        requirements: ScanRequires,
    ) -> Self {
        Self::new_with_all_options(scanner_id, repository_path, repository, requirements, None)
    }

    /// Create a new ScannerTask with all options
    pub fn new_with_all_options(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
        requirements: ScanRequires,
        query_params: Option<QueryParams>,
    ) -> Self {
        Self {
            scanner_id,
            repository_path: repository_path.clone(),
            repository,
            is_remote: repository_path.contains("://") || !repository_path.starts_with('/'),
            requirements,
            query_params,
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

    /// Get the scanner requirements
    pub fn requirements(&self) -> ScanRequires {
        self.requirements
    }

    /// Get the query parameters
    pub fn query_params(&self) -> Option<&QueryParams> {
        self.query_params.as_ref()
    }

    // Phase 7: Scanner Filters and Query Parameters
    /// Apply scanning filters based on query parameters
    pub async fn apply_scan_filters(&self, _query_params: QueryParams) -> ScanResult<()> {
        // Phase 7 placeholder - will implement filtering logic
        Ok(())
    }

    // Phase 8: Advanced Git Operations
    /// Perform advanced git operations for comprehensive scanning
    pub async fn perform_advanced_git_operations(&self) -> ScanResult<Vec<String>> {
        // Phase 8 placeholder - will implement advanced git operations
        Ok(vec!["advanced-operation-1".to_string()])
    }

    // Phase 9: Integration Testing and Polish
    /// Run integration tests for scanner functionality
    pub async fn run_integration_tests(&self) -> ScanResult<bool> {
        // Phase 9 placeholder - will implement integration testing
        Ok(true)
    }
}
