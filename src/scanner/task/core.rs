//! Scanner Task Core Implementation
//!
//! Core ScannerTask struct and basic methods including constructors and accessors.

use crate::core::query::QueryParams;
use crate::scanner::error::ScanResult;
use gix;
use std::sync::Arc;

/// Individual scanner task for a specific repository
#[derive(Clone)]
pub struct ScannerTask {
    /// Unique scanner ID (scan-<sha256>)
    scanner_id: String,
    /// Repository path (local or remote URL - normalized)
    repository_path: String,
    /// Cached thread-safe repository instance
    repository: Option<Arc<gix::ThreadSafeRepository>>,
    /// Flag indicating if this is a remote repository
    is_remote: bool,
}

impl ScannerTask {
    /// Create a new ScannerTask with cached repository instance
    pub fn new_with_cache(
        scanner_id: String,
        repository_path: String,
        repository: Arc<gix::ThreadSafeRepository>,
    ) -> Self {
        Self {
            scanner_id,
            repository_path: repository_path.clone(),
            repository: Some(repository),
            is_remote: repository_path.contains("://") || !repository_path.starts_with('/'),
        }
    }

    /// Create a new ScannerTask for a repository (for testing)
    pub async fn new(
        manager: &crate::scanner::ScannerManager,
        repository_path: &str,
    ) -> ScanResult<Self> {
        // This method is primarily for testing - production code should use create_scanner

        // Check if this is a remote URL BEFORE normalizing (since normalization strips protocols)
        if repository_path.contains("://") {
            // For remote URLs, use the original URL as the repo_id and normalize it
            log::warn!(
                "Remote repository URLs are not yet fully supported: {}",
                repository_path
            );
            let normalized_path = manager.normalise_repository_path(repository_path)?;
            let repo_id = repository_path.to_string(); // Use original URL as repo_id
            let scanner_id = manager.generate_scanner_id(&repo_id)?;

            return Ok(Self {
                scanner_id,
                repository_path: normalized_path,
                repository: None,
                is_remote: true,
            });
        }

        let normalized_path = manager.normalise_repository_path(repository_path)?;

        // For local repositories, validate and cache
        let path = std::path::Path::new(&normalized_path);
        let (repo, _git_dir) = manager.validate_repository(path)?;

        // Get the unique repository ID
        let repo_id = manager.get_unique_repo_id(&repo)?;

        // Generate scanner ID from the unique repo ID
        let scanner_id = manager.generate_scanner_id(&repo_id)?;

        // Convert to thread-safe repository for caching
        let thread_safe_repo = repo.into_sync();

        Ok(Self {
            scanner_id,
            repository_path: normalized_path,
            repository: Some(Arc::new(thread_safe_repo)),
            is_remote: false,
        })
    }

    /// Get the scanner ID
    pub fn scanner_id(&self) -> &str {
        &self.scanner_id
    }

    /// Get the repository path
    pub fn repository_path(&self) -> &str {
        &self.repository_path
    }

    /// Get reference to the cached repository instance
    pub(super) fn repository(&self) -> &Option<Arc<gix::ThreadSafeRepository>> {
        &self.repository
    }

    /// Check if this is a remote repository
    pub(super) fn is_remote(&self) -> bool {
        self.is_remote
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
