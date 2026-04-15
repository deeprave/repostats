//! Scanner Task Core Implementation
//!
//! Core ScannerTask struct and basic methods including constructors and accessors.

use crate::core::query::QueryParams;
use crate::notifications::api::{get_notification_service_arc, AsyncNotificationManager};
use crate::queue::api::QueuePublisher;
use crate::scanner::types::ScanRequires;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;

/// Individual scanner task for a specific repository
pub struct ScannerTask {
    /// Unique scanner ID (<16_char_sha256>)
    /// Uses first 16 characters of SHA256 hash for strong collision resistance while maintaining
    /// sufficient uniqueness for typical repository scanning workflows.
    /// Collision probability: ~1 in 16^16 (2^64) - extremely low for practical use cases.
    scanner_id: String,
    /// Repository path (local or remote URL - normalized)
    repository_path: String,
    /// Git repository instance
    repository: gix::ThreadSafeRepository,
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
    /// Injected notification manager (independent from global service)
    pub(crate) notification_manager: Arc<TokioMutex<AsyncNotificationManager>>,
    /// Root directory of checkout (set only once for target commit). Wrapped in Mutex for interior mutability
    pub(crate) checkout_root: Mutex<Option<std::path::PathBuf>>,
    /// Files for which we've already attached checkout_path (newest -> oldest traversal semantics). Mutex to allow mutation with &self
    pub(crate) seen_checkout_files: Mutex<std::collections::HashSet<String>>,
}

impl std::fmt::Debug for ScannerTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScannerTask")
            .field("scanner_id", &self.scanner_id)
            .field("repository_path", &self.repository_path)
            .field("is_remote", &self.is_remote)
            .field("requirements", &self.requirements)
            .field("query_params", &self.query_params)
            .field("checkout_manager", &self.checkout_manager)
            .field("queue_publisher", &self.queue_publisher)
            .field("notification_manager", &"<AsyncNotificationManager>")
            .field("checkout_root", &self.checkout_root)
            .field("seen_checkout_files", &self.seen_checkout_files)
            .finish()
    }
}

impl ScannerTask {
    /// Create a new `ScannerTask` builder with the required runtime dependencies.
    pub fn builder<R>(
        scanner_id: String,
        repository_path: String,
        repository: R,
        queue_publisher: QueuePublisher,
    ) -> ScannerTaskBuilder
    where
        R: Into<gix::ThreadSafeRepository>,
    {
        ScannerTaskBuilder {
            scanner_id,
            repository_path,
            repository: repository.into(),
            requirements: ScanRequires::NONE,
            query_params: None,
            checkout_manager: None,
            queue_publisher,
            notification_manager: None,
        }
    }

    fn from_builder(builder: ScannerTaskBuilder) -> Self {
        let is_remote = Self::is_remote_path(&builder.repository_path);

        Self {
            scanner_id: builder.scanner_id,
            repository_path: builder.repository_path,
            repository: builder.repository,
            is_remote,
            requirements: builder.requirements,
            query_params: builder.query_params,
            checkout_manager: builder.checkout_manager,
            queue_publisher: builder.queue_publisher,
            notification_manager: builder
                .notification_manager
                .unwrap_or_else(get_notification_service_arc),
            checkout_root: Mutex::new(None),
            seen_checkout_files: Mutex::new(std::collections::HashSet::new()),
        }
    }

    /// Create a concise test-only builder that auto-creates a queue publisher.
    #[cfg(test)]
    pub fn builder_for_tests(
        scanner_id: String,
        repository_path: String,
        repository: gix::Repository,
    ) -> ScannerTaskBuilder {
        let queue_service = crate::queue::api::get_queue_service();
        let test_publisher = queue_service
            .create_publisher(scanner_id.clone())
            .expect("Failed to create test queue publisher");

        Self::builder(scanner_id, repository_path, repository, test_publisher)
    }
}

/// Builder for constructing a fully initialized `ScannerTask`.
pub struct ScannerTaskBuilder {
    scanner_id: String,
    repository_path: String,
    repository: gix::ThreadSafeRepository,
    requirements: ScanRequires,
    query_params: Option<QueryParams>,
    checkout_manager: Option<Arc<Mutex<crate::scanner::checkout::manager::CheckoutManager>>>,
    queue_publisher: QueuePublisher,
    notification_manager: Option<Arc<TokioMutex<AsyncNotificationManager>>>,
}

impl ScannerTaskBuilder {
    pub fn with_requirements(mut self, requirements: ScanRequires) -> Self {
        self.requirements = requirements;
        self
    }

    pub fn with_query_params(mut self, query_params: Option<QueryParams>) -> Self {
        self.query_params = query_params;
        self
    }

    pub fn with_checkout_manager(
        mut self,
        checkout_manager: Option<Arc<Mutex<crate::scanner::checkout::manager::CheckoutManager>>>,
    ) -> Self {
        self.checkout_manager = checkout_manager;
        self
    }

    pub fn with_notification_manager(
        mut self,
        notification_manager: Arc<TokioMutex<AsyncNotificationManager>>,
    ) -> Self {
        self.notification_manager = Some(notification_manager);
        self
    }

    pub fn build(self) -> ScannerTask {
        ScannerTask::from_builder(self)
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
        // Create a test queue publisher and notification manager for test scenarios
        let queue_service = crate::queue::api::get_queue_service();
        let test_publisher = queue_service
            .create_publisher(scanner_id.clone())
            .expect("Failed to create test queue publisher");

        Self::builder(scanner_id, repository_path, repository, test_publisher).build()
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

    /// Materialize a thread-local repository handle from the shared repository state.
    ///
    /// Callers should cache the returned handle in a local variable when they need
    /// to perform multiple repository operations within the same function.
    pub fn repository(&self) -> gix::Repository {
        self.repository.to_thread_local()
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
