//! Scanner Manager
//!
//! Central coordination component for managing multiple repository scanner tasks,
//! each with unique SHA256-based identification to prevent duplicate scanning.

use crate::core::cleanup::Cleanup;
use crate::core::query::QueryParams;
use crate::scanner::checkout::manager::CheckoutManager;
use crate::scanner::error::{ScanError, ScanResult};
use crate::scanner::task::ScannerTask;
use log;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Repository reservation state
#[derive(Debug, Clone)]
enum RepoState {
    /// Repository is confirmed as being scanned
    Active,
    /// Repository is reserved for scanning (with timestamp for cleanup)
    Reserved(Instant),
}

/// Checkout state tracking
#[derive(Debug)]
struct CheckoutState {
    /// The CheckoutManager instance (shared with thread safety)
    manager: Arc<Mutex<CheckoutManager>>,
}

/// Central scanner manager for coordinating multiple repository scanner tasks
pub struct ScannerManager {
    /// Active scanner tasks by repository hash
    _scanner_tasks: Mutex<HashMap<String, Arc<ScannerTask>>>, // hash -> scanner task
    /// Repository states to prevent duplicate scanners with reservation system
    repo_states: Mutex<HashMap<String, RepoState>>,
    /// Checkout managers by scanner ID for file system checkouts
    checkout_managers: Mutex<HashMap<String, CheckoutState>>,
    /// Mapping of plugin ID to scanner IDs they're using checkouts from
    plugin_to_scanners: Mutex<HashMap<String, HashSet<String>>>,
}

impl ScannerManager {
    /// Length of scanner ID hash portion (12 characters for balance of uniqueness and readability)
    const SCANNER_ID_HASH_LENGTH: usize = 12;

    /// Create a new ScannerManager instance
    pub fn new() -> Self {
        Self {
            _scanner_tasks: Mutex::new(HashMap::new()),
            repo_states: Mutex::new(HashMap::new()),
            checkout_managers: Mutex::new(HashMap::new()),
            plugin_to_scanners: Mutex::new(HashMap::new()),
        }
    }

    /// Create a ScannerManager and integrate with services
    pub async fn create() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Try to reserve a repository for scanning
    /// Returns true if reservation successful, false if already active/reserved
    fn try_reserve_repository(&self, repo_id: &str) -> bool {
        let mut repo_states = self.repo_states.lock().unwrap();

        // Clean up expired reservations (older than 30 seconds)
        let now = Instant::now();
        let expiry_threshold = Duration::from_secs(30);
        repo_states.retain(|_, state| {
            match state {
                RepoState::Active => true, // Keep active entries
                RepoState::Reserved(timestamp) => now.duration_since(*timestamp) < expiry_threshold,
            }
        });

        // Try to reserve if not already active or reserved
        match repo_states.get(repo_id) {
            Some(RepoState::Active) | Some(RepoState::Reserved(_)) => false,
            None => {
                repo_states.insert(repo_id.to_string(), RepoState::Reserved(now));
                true
            }
        }
    }

    /// Confirm a reservation by marking repository as active
    fn confirm_reservation(&self, repo_id: &str) -> bool {
        let mut repo_states = self.repo_states.lock().unwrap();
        match repo_states.get(repo_id) {
            Some(RepoState::Reserved(_)) => {
                repo_states.insert(repo_id.to_string(), RepoState::Active);
                true
            }
            _ => false, // Not reserved or already active
        }
    }

    /// Cancel a reservation
    fn cancel_reservation(&self, repo_id: &str) {
        let mut repo_states = self.repo_states.lock().unwrap();
        if let Some(RepoState::Reserved(_)) = repo_states.get(repo_id) {
            repo_states.remove(repo_id);
        }
    }

    /// Validate a repository path using gix and return the Repository and normalized path
    /// Also validates that specified git refs exist in the repository
    pub fn validate_repository(
        &self,
        repository_path: &Path,
        query_params: Option<&QueryParams>,
        checkout_settings: Option<&crate::app::cli::CheckoutSettings>,
    ) -> ScanResult<(gix::Repository, PathBuf)> {
        // For now, reject remote URLs
        let path_str = repository_path.to_string_lossy();
        if path_str.contains("://") {
            return Err(ScanError::Configuration {
                message: "Remote repository URLs are not yet supported".to_string(),
            });
        }

        // Attempt to discover and open the repository using gix
        match gix::discover(repository_path) {
            Ok(repo) => {
                // Validate git refs if specified
                if let Some(params) = query_params {
                    if let Some(ref git_ref) = params.git_ref {
                        self.validate_git_ref(&repo, git_ref, "--ref")?;
                    }
                }

                if let Some(settings) = checkout_settings {
                    if let Some(ref checkout_rev) = settings.default_revision {
                        self.validate_git_ref(&repo, checkout_rev, "--checkout-rev")?;
                    }
                }

                // Get the normalized path (the actual git directory)
                let git_dir = repo.git_dir().to_path_buf();

                // Try to canonicalize to resolve symlinks and normalize
                let normalized_path = git_dir.canonicalize().unwrap_or_else(|_| git_dir.clone());

                Ok((repo, normalized_path))
            }
            Err(e) => {
                // Repository validation failed
                Err(ScanError::Repository {
                    message: format!(
                        "Invalid repository at '{}': {}",
                        repository_path.display(),
                        e
                    ),
                })
            }
        }
    }

    /// Validate that a git reference exists in the repository
    fn validate_git_ref(
        &self,
        repo: &gix::Repository,
        git_ref: &str,
        flag_name: &str,
    ) -> ScanResult<()> {
        // Use gix to resolve the reference
        match repo.rev_parse(git_ref) {
            Ok(_) => Ok(()),
            Err(e) => Err(ScanError::Configuration {
                message: format!(
                    "Invalid git reference '{}' for {} flag in repository '{}': {}",
                    git_ref,
                    flag_name,
                    repo.workdir()
                        .or_else(|| repo.git_dir().parent())
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    e
                ),
            }),
        }
    }

    /// Normalise a repository path/URL for consistent hashing and deduplication
    pub fn normalise_repository_path(&self, repository_path: &str) -> ScanResult<String> {
        let path = repository_path.trim();

        // Check if it's a URL (contains scheme://)
        if let Some(scheme_end) = path.find("://") {
            // It's a remote URL - extract hostname + path only
            let after_scheme = &path[scheme_end + 3..];

            // Remove authentication info if present (user@host -> host)
            let host_path = if let Some(at_pos) = after_scheme.find('@') {
                &after_scheme[at_pos + 1..]
            } else {
                after_scheme
            };

            // Remove .git extension if present
            let normalised = if let Some(stripped) = host_path.strip_suffix(".git") {
                stripped
            } else {
                host_path
            };

            Ok(normalised.to_string())
        } else {
            // It's a local path - resolve to absolute path and remove .git extension
            let path_buf = PathBuf::from(path);

            // Try to canonicalize (resolve to absolute path)
            let absolute_path = match path_buf.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    // If canonicalize fails, just use the original path
                    path_buf
                }
            };

            let mut path_str = absolute_path.to_string_lossy().to_string();

            // Remove .git extension if present
            if path_str.ends_with(".git") {
                path_str = path_str[..path_str.len() - 4].to_string();
            }

            Ok(path_str)
        }
    }

    /// Get a unique repository ID for deduplication
    pub fn get_unique_repo_id(&self, repo: &gix::Repository) -> ScanResult<String> {
        // Try to get the origin remote URL first (most unique for clones)
        let config = repo.config_snapshot();
        if let Some(remote_url) = config.string("remote.origin.url") {
            return Ok(remote_url.to_string());
        }

        // Fallback to canonical git directory path
        let git_dir = repo.git_dir();
        git_dir
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .or_else(|_| Ok(git_dir.to_string_lossy().to_string()))
    }

    /// Generate SHA256-based scanner ID for a repository (now using repo_id)
    ///
    /// Creates a 12-character truncated SHA256 hash providing sufficient uniqueness
    /// (collision probability ~1 in 281 trillion) while maintaining readability.
    pub fn generate_scanner_id(&self, repo_id: &str) -> ScanResult<String> {
        // Generate SHA256 hash of the unique repo ID
        let mut hasher = Sha256::new();
        hasher.update(repo_id.as_bytes());
        let hash_result = hasher.finalize();

        // Convert to hex string and truncate to configured length for readability
        // SHA256 always produces 64 hex characters, so truncation is always safe
        let hash_hex = format!("{:x}", hash_result);
        let truncated_hash = &hash_hex[..Self::SCANNER_ID_HASH_LENGTH];
        Ok(format!("scan-{}", truncated_hash))
    }

    /// Create a scanner for a repository with queue integration
    pub async fn create_scanner(
        &self,
        repository_path: &str,
        query_params: Option<&QueryParams>,
        checkout_settings: Option<&crate::app::cli::CheckoutSettings>,
    ) -> ScanResult<Arc<ScannerTask>> {
        // First normalize the path
        let normalized_path = self.normalise_repository_path(repository_path)?;

        // Validate the repository and get the gix::Repository instance
        let path = Path::new(&normalized_path);
        let (repo, _git_dir) = self.validate_repository(path, query_params, checkout_settings)?;

        // Get the unique repository ID
        let repo_id = self.get_unique_repo_id(&repo)?;

        // Try to reserve the repository for scanning (atomic operation)
        if !self.try_reserve_repository(&repo_id) {
            return Err(ScanError::Configuration {
                message: format!(
                    "Repository '{}' is already being scanned (duplicate detected via {})",
                    repository_path,
                    if repo_id.contains("://") {
                        "remote URL"
                    } else {
                        "git directory"
                    }
                ),
            });
        }

        // Generate scanner ID from the unique repo ID
        let scanner_id = match self.generate_scanner_id(&repo_id) {
            Ok(id) => id,
            Err(e) => {
                // Cancel reservation on failure
                self.cancel_reservation(&repo_id);
                return Err(e);
            }
        };

        // Query all active plugins for their requirements
        let plugin_manager = crate::plugin::api::get_plugin_service().await;
        let requirements = plugin_manager.get_combined_requirements().await;

        // Check if checkout is needed BEFORE creating scanner task
        let needs_checkout = checkout_settings.is_some() || requirements.requires_file_content();

        // Create checkout manager first if needed, so we can inject it
        let checkout_manager = if needs_checkout {
            // Create the checkout manager using existing logic
            let checkout_manager = if let Some(ref settings) = checkout_settings {
                CheckoutManager::with_settings(
                    &normalized_path,
                    settings.checkout_template.clone(),
                    settings.keep_checkouts,
                    settings.force_overwrite,
                    settings.default_revision.clone(),
                )
            } else {
                // Create default checkout manager for FILE_CONTENT requirement
                let template = Some(format!(
                    "{}/repostats-checkout-{}/{{commit-id}}",
                    std::env::temp_dir().display(),
                    &scanner_id
                ));
                CheckoutManager::with_settings(
                    &normalized_path,
                    template,
                    false, // don't keep files by default
                    true,  // force overwrite
                    None,  // no default revision
                )
            };

            // Wrap in Arc<Mutex<>> for shared access
            let shared_manager = Arc::new(Mutex::new(checkout_manager));

            // Store checkout manager in centralized tracking
            let checkout_state = CheckoutState {
                manager: shared_manager.clone(),
            };

            self.checkout_managers
                .lock()
                .unwrap()
                .insert(scanner_id.clone(), checkout_state);
            Some(shared_manager)
        } else {
            None
        };

        // Create shared queue publisher with retry utility
        use crate::core::retry::{retry_async, RetryPolicy};
        let queue_publisher = retry_async("create_queue_publisher", RetryPolicy::default(), || {
            let scanner_id = scanner_id.clone();
            async move { crate::queue::api::get_queue_service().create_publisher(scanner_id) }
        })
        .await
        .map_err(|e| {
            // Cancel reservation on failure
            self.cancel_reservation(&repo_id);
            ScanError::Configuration {
                message: format!(
                    "Failed to create queue publisher for '{}' after {} attempts: {}",
                    repository_path,
                    RetryPolicy::default().max_attempts,
                    e
                ),
            }
        })?;

        // Create scanner task with simple dependency injection
        let scanner_task = ScannerTask::new(
            scanner_id.clone(),
            normalized_path.clone(),
            repo,
            requirements,
            queue_publisher,
            query_params.cloned(),
            checkout_manager,
        );
        let scanner_task = Arc::new(scanner_task);

        // Store the scanner task in the manager for later use
        self._scanner_tasks
            .lock()
            .unwrap()
            .insert(scanner_id.clone(), scanner_task.clone());

        // Create notification subscriber with retry utility
        let _subscriber = retry_async(
            "create_notification_subscriber",
            RetryPolicy::default(),
            || scanner_task.create_notification_subscriber(),
        )
        .await
        .map_err(|e| {
            // Cancel reservation on failure
            self.cancel_reservation(&repo_id);
            ScanError::Configuration {
                message: format!(
                    "Failed to create notification subscriber for '{}' after {} attempts: {}",
                    repository_path,
                    RetryPolicy::default().max_attempts,
                    e
                ),
            }
        })?;

        // Confirm the reservation now that all async operations succeeded
        if !self.confirm_reservation(&repo_id) {
            // This should not happen unless there was a reservation timeout
            return Err(ScanError::Configuration {
                message: format!(
                    "Repository reservation expired for '{}'. Please retry.",
                    repository_path
                ),
            });
        }

        Ok(scanner_task.clone())
    }

    /// Create scanners for multiple repositories with all-or-nothing semantics
    ///
    /// This method takes a list of repository paths and query parameters, and creates
    /// scanners for all of them. If ANY repository fails validation or scanner creation,
    /// all successfully created scanners are cleaned up and an error is returned.
    ///
    /// This ensures that startup either succeeds completely or fails completely,
    /// avoiding partial initialization states.
    pub async fn create_scanners(
        &self,
        repository_paths: &[PathBuf],
        query_params: Option<&QueryParams>,
        checkout_settings: Option<&crate::app::cli::CheckoutSettings>,
    ) -> ScanResult<Vec<Arc<ScannerTask>>> {
        // Note: Empty repository list is handled by startup layer which defaults to current directory
        // so this method should never receive an empty list in normal operation

        let mut created_scanners = Vec::new();
        let mut failed_repositories = Vec::new();

        // Try to create scanners for all repositories
        for (index, repo_path) in repository_paths.iter().enumerate() {
            let repo_path_str = repo_path.to_string_lossy();

            match self
                .create_scanner(&repo_path_str, query_params, checkout_settings)
                .await
            {
                Ok(scanner) => {
                    log::info!(
                        "Successfully created scanner for repository '{}' (#{}/{})",
                        repo_path_str,
                        index + 1,
                        repository_paths.len()
                    );
                    created_scanners.push(scanner);
                }
                Err(e) => {
                    log::error!(
                        "Failed to create scanner for repository '{}' (#{}/{}): {}",
                        repo_path_str,
                        index + 1,
                        repository_paths.len(),
                        e
                    );
                    failed_repositories.push((repo_path_str.to_string(), e));
                    break; // Stop on first failure for all-or-nothing semantics
                }
            }
        }

        // If any repository failed, clean up all successfully created scanners
        if !failed_repositories.is_empty() {
            log::warn!(
                "Scanner creation failed - cleaning up {} successfully created scanners",
                created_scanners.len()
            );

            // Clean up successfully created scanners
            for scanner in &created_scanners {
                let scanner_id = scanner.scanner_id();

                // Remove from active scanners map
                self._scanner_tasks.lock().unwrap().remove(scanner_id);

                // Cancel repository reservation if applicable
                if let Ok(repo_id) = self.get_unique_repo_id(scanner.repository()) {
                    self.cancel_reservation(&repo_id);
                }

                log::trace!("Cleaned up scanner: {}", scanner_id);
            }

            // Return detailed error about the failure
            let (failed_repo, failed_error) = &failed_repositories[0];
            return Err(ScanError::Configuration {
                message: format!(
                    "Repository scanning initialization failed: '{}' - {}. {} scanners cleaned up.",
                    failed_repo,
                    failed_error,
                    created_scanners.len()
                ),
            });
        }

        log::info!(
            "Successfully created {} scanners for all repositories",
            created_scanners.len()
        );

        Ok(created_scanners)
    }

    /// Start scanning all configured repositories
    /// This triggers scan_commits_and_publish_incrementally() on all scanner tasks and waits for completion
    pub async fn start_scanning(&self) -> Result<(), ScanError> {
        // Collect all scanner tasks first, then drop the lock
        let scanner_tasks_vec = {
            let scanner_tasks = self._scanner_tasks.lock().unwrap();

            if scanner_tasks.is_empty() {
                return Err(ScanError::Repository {
                    message: "No active repository scanner".to_string(),
                });
            }

            log::trace!(
                "Starting repository scanning for {} repositories",
                scanner_tasks.len()
            );

            // Collect all scanner tasks into a vector
            scanner_tasks
                .iter()
                .map(|(id, task)| (id.clone(), task.clone()))
                .collect::<Vec<_>>()
        }; // Lock is dropped here

        // Process all repositories sequentially (due to thread safety constraints of gix::Repository)
        let mut success_count = 0;
        let mut failure_count = 0;

        for (scanner_id, scanner_task) in scanner_tasks_vec {
            log::trace!("Starting scan for scanner: {}", scanner_id);

            // Use incremental publishing to avoid memory buildup for large repositories
            match scanner_task.scan_commits_and_publish_incrementally().await {
                Ok(()) => {
                    success_count += 1;
                    log::trace!(
                        "Successfully scanned and published messages for scanner: {}",
                        scanner_id
                    );
                }
                Err(e) => {
                    failure_count += 1;
                    log::error!(
                        "Failed to scan and publish for scanner '{}': {}",
                        scanner_id,
                        e
                    );
                }
            }
        }

        log::info!(
            "Repository scanning completed: {} successful, {} failed",
            success_count,
            failure_count
        );

        if success_count == 0 {
            return Err(ScanError::Configuration {
                message: "All repository scans failed".to_string(),
            });
        }

        Ok(())
    }

    /// Get the current number of active scanners (for testing)
    #[cfg(test)]
    pub fn scanner_count(&self) -> usize {
        self._scanner_tasks.lock().unwrap().len()
    }

    // ===== Checkout Management Methods =====

    /// Cleanup all checkouts when shutting down
    pub fn cleanup_all_checkouts(&self) {
        let mut checkout_managers = self.checkout_managers.lock().unwrap();

        for (scanner_id, state) in checkout_managers.drain() {
            if !state.manager.lock().unwrap().keep_files {
                if let Err(e) = state.manager.lock().unwrap().cleanup_all() {
                    log::warn!(
                        "Failed to cleanup checkouts for scanner '{}': {}",
                        scanner_id,
                        e
                    );
                } else {
                    log::trace!("Cleaned up checkouts for scanner '{}'", scanner_id);
                }
            }
        }

        // Clear plugin mappings
        let mut plugin_to_scanners = self.plugin_to_scanners.lock().unwrap();
        plugin_to_scanners.clear();
    }

    /// Get an opaque cleanup handle for main.rs coordination
    ///
    /// Returns a trait object that allows main.rs to trigger cleanup operations
    /// without needing to know about internal management structure.
    pub fn cleanup_handle(self: Arc<Self>) -> Arc<dyn Cleanup> {
        self
    }
}

/// Implementation of Cleanup trait for ScannerManager
impl Cleanup for ScannerManager {
    fn cleanup(&self) {
        // Delegate to the existing implementation
        let mut checkout_managers = self.checkout_managers.lock().unwrap();

        for (scanner_id, state) in checkout_managers.drain() {
            // Acquire the manager lock once to avoid potential deadlock
            let mut manager = state.manager.lock().unwrap();
            if !manager.keep_files {
                if let Err(e) = manager.cleanup_all() {
                    log::trace!(
                        "Failed to cleanup checkouts for scanner '{}': {}",
                        scanner_id,
                        e
                    );
                }
            }
        }

        // Clear plugin mappings
        let mut plugin_to_scanners = self.plugin_to_scanners.lock().unwrap();
        plugin_to_scanners.clear();
    }
}

impl Drop for ScannerManager {
    fn drop(&mut self) {
        // Cleanup all checkouts when the manager is dropped
        // This ensures cleanup even on panic or unexpected termination
        let checkout_managers = self.checkout_managers.lock().unwrap();
        let active_count = checkout_managers.len();

        if active_count > 0 {
            log::info!(
                "ScannerManager dropping: cleaning up {} active checkout managers",
                active_count
            );
            drop(checkout_managers); // Release lock before cleanup
            self.cleanup_all_checkouts();
        }
    }
}
