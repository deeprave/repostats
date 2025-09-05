//! Scanner Manager
//!
//! Central coordination component for managing multiple repository scanner tasks,
//! each with unique SHA256-based identification to prevent duplicate scanning.

use crate::core::cleanup::Cleanup;
use crate::core::query::QueryParams;
use crate::core::retry::RetryPolicy;
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
    /// Track if cleanup has already occurred to prevent double cleanup
    cleanup_done: Mutex<bool>,
    /// Override for filesystem case sensitivity detection (None = use platform heuristic)
    case_insensitive_override: Option<bool>,
}

impl ScannerManager {
    /// Length of scanner ID hash portion (16 characters for strong collision resistance)
    const SCANNER_ID_HASH_LENGTH: usize = 16;

    /// Reservation timeout for repository scanning
    const RESERVATION_TIMEOUT: Duration = Duration::from_secs(30);

    /// Retry policy for local queue operations
    const QUEUE_RETRY_POLICY: RetryPolicy = RetryPolicy {
        max_attempts: 2,
        delay: Duration::from_millis(100),
    };

    /// Emergency cleanup timeout for checkout managers
    const EMERGENCY_CLEANUP_TIMEOUT: Duration = Duration::from_millis(500);

    /// Helper method to handle poisoned mutex cases (wraps core utility)
    fn handle_mutex_poison<T>(result: std::sync::LockResult<T>) -> ScanResult<T> {
        crate::core::sync::handle_mutex_poison(result, |msg| ScanError::Configuration {
            message: msg,
        })
    }

    /// Create a new ScannerManager instance
    pub fn new() -> Self {
        Self {
            _scanner_tasks: Mutex::new(HashMap::new()),
            repo_states: Mutex::new(HashMap::new()),
            checkout_managers: Mutex::new(HashMap::new()),
            plugin_to_scanners: Mutex::new(HashMap::new()),
            cleanup_done: Mutex::new(false),
            case_insensitive_override: None,
        }
    }

    /// Create a new ScannerManager instance with case sensitivity override
    pub fn with_case_sensitivity(case_insensitive_override: Option<bool>) -> Self {
        Self {
            _scanner_tasks: Mutex::new(HashMap::new()),
            repo_states: Mutex::new(HashMap::new()),
            checkout_managers: Mutex::new(HashMap::new()),
            plugin_to_scanners: Mutex::new(HashMap::new()),
            cleanup_done: Mutex::new(false),
            case_insensitive_override,
        }
    }

    /// Create a ScannerManager and integrate with services
    pub async fn create() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Try to reserve a repository for scanning
    /// Returns true if reservation successful, false if already active/reserved
    fn try_reserve_repository(&self, repo_id: &str) -> bool {
        let mut repo_states = match Self::handle_mutex_poison(self.repo_states.lock()) {
            Ok(guard) => guard,
            Err(e) => {
                log::error!("Failed to acquire repo_states lock: {}", e);
                return false;
            }
        };

        // Clean up expired reservations (older than reservation timeout)
        self.cleanup_expired_reservations_internal(&mut repo_states);

        // Try to reserve if not already active or reserved
        match repo_states.get(repo_id) {
            Some(RepoState::Active) | Some(RepoState::Reserved(_)) => false,
            None => {
                let now = Instant::now();
                repo_states.insert(repo_id.to_string(), RepoState::Reserved(now));
                true
            }
        }
    }

    /// Confirm a reservation by marking repository as active
    fn confirm_reservation(&self, repo_id: &str) -> bool {
        let mut repo_states = match Self::handle_mutex_poison(self.repo_states.lock()) {
            Ok(guard) => guard,
            Err(e) => {
                log::error!("Failed to acquire repo_states lock: {}", e);
                return false;
            }
        };
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
        let mut repo_states = match Self::handle_mutex_poison(self.repo_states.lock()) {
            Ok(guard) => guard,
            Err(e) => {
                log::error!("Failed to acquire repo_states lock for cancellation: {}", e);
                return;
            }
        };
        if let Some(RepoState::Reserved(_)) = repo_states.get(repo_id) {
            repo_states.remove(repo_id);
        }
    }

    /// Internal helper for cleaning up expired reservations
    fn cleanup_expired_reservations_internal(&self, repo_states: &mut HashMap<String, RepoState>) {
        let now = Instant::now();
        let expiry_threshold = Self::RESERVATION_TIMEOUT;
        repo_states.retain(|_, state| {
            match state {
                RepoState::Active => true, // Keep active entries
                RepoState::Reserved(timestamp) => now.duration_since(*timestamp) < expiry_threshold,
            }
        });
    }

    /// Normalize path case for case-insensitive filesystems
    ///
    /// On case-insensitive filesystems (Windows, macOS default), paths that
    /// differ only in case refer to the same location.
    /// This method normalizes paths to lowercase on such systems
    /// to ensure consistent deduplication.
    fn normalize_path_case(&self, path: &str) -> String {
        // Determine if we're likely on a case-insensitive filesystem
        if self.is_case_insensitive_filesystem() {
            // Convert to lowercase for consistent comparison
            // This handles the common case where paths differ only in case
            path.to_lowercase()
        } else {
            // On case-sensitive filesystems, preserve original case
            path.to_string()
        }
    }

    /// Check if we're likely on a case-insensitive filesystem, or use configured override
    ///
    /// This is a heuristic based on the target platform, as direct
    /// filesystem capability detection would be complex and potentially slow.
    /// The override allows handling edge cases where platforms differ from defaults.
    fn is_case_insensitive_filesystem(&self) -> bool {
        if let Some(override_value) = self.case_insensitive_override {
            return override_value;
        }

        // Windows filesystems (NTFS, FAT32) are case-insensitive by default
        #[cfg(target_os = "windows")]
        return true;

        // macOS filesystems can be case-insensitive (HFS+, APFS default)
        // We err on the side of caution and assume case-insensitive
        #[cfg(target_os = "macos")]
        return true;

        // Linux and other Unix systems are typically case-sensitive
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        return false;
    }

    /// Validate a repository path using gix and return the Repository and normalised path
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

                // Get the normalised path (the actual git directory)
                let git_dir = repo.git_dir().to_path_buf();

                // Try to canonicalise to resolve symlinks and normalise
                let normalised_path = git_dir.canonicalize().unwrap_or_else(|_| git_dir.clone());

                Ok((repo, normalised_path))
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
    /// Handles:
    /// - Trimming whitespace
    /// - Removing trailing slashes
    /// - Lowercasing hostnames in URLs
    /// - Removing authentication info and .git extensions
    /// - Normalising port specifications
    /// - Handling scp-like git URLs (e.g., git@host:path/repo.git)
    pub fn normalise_repository_path(&self, repository_path: &str) -> ScanResult<String> {
        let path = repository_path.trim().trim_end_matches('/');

        // Handle scp-like URLs: git@host:path/repo(.git)
        if let Some(idx) = path.find(':') {
            // Check for scp-like pattern (user@host:path)
            if path[..idx].contains('@') && !path[..idx].contains('/') {
                let mut parts = path.splitn(2, ':');
                let user_host = parts.next().unwrap();
                let repo_path = parts.next().unwrap();

                // Remove authentication info (user@)
                let host = user_host.split('@').last().unwrap().to_lowercase();

                // Remove .git extension if present
                let repo_path = repo_path.trim_end_matches(".git");

                // Normalise: host:path
                return Ok(format!("{}:{}", host, repo_path));
            }
        }

        // Check if it's a URL (contains scheme://)
        if let Some(scheme_end) = path.find("://") {
            let scheme = &path[..scheme_end].to_ascii_lowercase();
            let after_scheme = &path[scheme_end + 3..];

            // Remove authentication info if present (user@host -> host)
            let host_path = if let Some(at_pos) = after_scheme.find('@') {
                &after_scheme[at_pos + 1..]
            } else {
                after_scheme
            };

            // Parse host and path components
            let (host_with_port, repo_path) = if let Some(slash_pos) = host_path.find('/') {
                (&host_path[..slash_pos], &host_path[slash_pos..])
            } else {
                (host_path, "")
            };

            // Separate host from port and normalise
            let normalised_host = if let Some(colon_pos) = host_with_port.find(':') {
                let host = &host_with_port[..colon_pos];
                let port = &host_with_port[colon_pos + 1..];

                // Only include non-default ports
                match (scheme.as_str(), port) {
                    ("http", "80") | ("https", "443") | ("ssh", "22") => host.to_ascii_lowercase(),
                    _ => format!("{}:{}", host.to_ascii_lowercase(), port),
                }
            } else {
                host_with_port.to_ascii_lowercase()
            };

            // Clean up the repository path
            let clean_repo_path = repo_path
                .trim_end_matches('/')
                .strip_suffix(".git")
                .unwrap_or(repo_path.trim_end_matches('/'));

            // Reconstruct the normalised URL
            let normalised = if clean_repo_path.is_empty() {
                normalised_host
            } else {
                format!("{}{}", normalised_host, clean_repo_path)
            };

            Ok(normalised)
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

            // Normalise path separators and remove trailing slashes
            let normalised = path_str.trim_end_matches('/').trim_end_matches('\\');

            // Handle case sensitivity based on filesystem characteristics
            let final_normalised = self.normalize_path_case(normalised);

            Ok(final_normalised)
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
    /// Creates a 16-character truncated SHA256 hash providing strong collision resistance.
    /// With 64 bits of hash space (2^64), collisions become likely only after ~4.3 billion
    /// different repositories due to birthday paradox, making this safe for practical use.
    pub fn generate_scanner_id(&self, repo_id: &str) -> ScanResult<String> {
        // Generate SHA256 hash of the unique repo ID
        let mut hasher = Sha256::new();
        hasher.update(repo_id.as_bytes());
        let hash_result = hasher.finalize();

        // Convert to hex string and truncate to configured length for readability
        // SHA256 always produces 64 hex characters, so truncation is always safe
        let hash_hex = format!("{:x}", hash_result);

        // Ensure we don't exceed the actual hash length (defensive programming)
        let truncate_length = std::cmp::min(Self::SCANNER_ID_HASH_LENGTH, hash_hex.len());
        let truncated_hash = &hash_hex[..truncate_length];
        Ok(format!("scan-{}", truncated_hash))
    }

    /// Create a scanner for a repository with queue integration
    pub async fn create_scanner(
        &self,
        repository_path: &str,
        query_params: Option<&QueryParams>,
        checkout_settings: Option<&crate::app::cli::CheckoutSettings>,
    ) -> ScanResult<Arc<ScannerTask>> {
        // First normalise the path
        let normalised_path = self.normalise_repository_path(repository_path)?;

        // Validate the repository and get the gix::Repository instance
        let path = Path::new(&normalised_path);
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
                    settings.checkout_template.clone().unwrap_or_else(|| {
                        format!(
                            "{}/repostats-checkout-{}/{{commit-id}}",
                            std::env::temp_dir().display(),
                            &scanner_id
                        )
                    }),
                    settings.keep_checkouts,
                    settings.force_overwrite,
                )
            } else {
                // Create default checkout manager for FILE_CONTENT requirement
                let template = Some(format!(
                    "{}/repostats-checkout-{}/{{commit-id}}",
                    std::env::temp_dir().display(),
                    &scanner_id
                ));
                CheckoutManager::with_settings(
                    template.unwrap_or_else(|| {
                        format!(
                            "{}/repostats-checkout-{}/{{commit-id}}",
                            std::env::temp_dir().display(),
                            &scanner_id
                        )
                    }),
                    false, // don't keep files by default
                    true,  // force overwrite
                )
            };

            // Wrap in Arc<Mutex<>> for shared access
            let shared_manager = Arc::new(Mutex::new(checkout_manager));

            // Store checkout manager in centralized tracking
            let checkout_state = CheckoutState {
                manager: shared_manager.clone(),
            };

            match Self::handle_mutex_poison(self.checkout_managers.lock()) {
                Ok(mut managers) => {
                    managers.insert(scanner_id.clone(), checkout_state);
                }
                Err(e) => {
                    self.cancel_reservation(&repo_id);
                    return Err(ScanError::Configuration {
                        message: format!("Failed to register checkout manager: {}", e),
                    });
                }
            }
            Some(shared_manager)
        } else {
            None
        };

        // Create shared queue publisher with optimized retry policy for local operations
        use crate::core::retry::retry_async;

        // Use configured retry policy for queue operations - failures are likely permanent
        let queue_publisher =
            retry_async("create_queue_publisher", Self::QUEUE_RETRY_POLICY, || {
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
                        Self::QUEUE_RETRY_POLICY.max_attempts,
                        e
                    ),
                }
            })?;

        // Create scanner task with simple dependency injection
        let scanner_task = ScannerTask::new(
            scanner_id.clone(),
            normalised_path.clone(),
            repo,
            requirements,
            queue_publisher,
            query_params.cloned(),
            checkout_manager,
        );
        let scanner_task = Arc::new(scanner_task);

        // Store the scanner task in the manager for later use
        match Self::handle_mutex_poison(self._scanner_tasks.lock()) {
            Ok(mut tasks) => {
                tasks.insert(scanner_id.clone(), scanner_task.clone());
            }
            Err(e) => {
                self.cancel_reservation(&repo_id);
                return Err(ScanError::Configuration {
                    message: format!("Failed to register scanner task: {}", e),
                });
            }
        }

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
                if let Ok(mut tasks) = Self::handle_mutex_poison(self._scanner_tasks.lock()) {
                    tasks.remove(scanner_id);
                } else {
                    log::error!("Failed to acquire lock for scanner cleanup");
                }

                // Cancel repository reservation if applicable
                if let Ok(repo_id) = self.get_unique_repo_id(scanner.repository()) {
                    self.cancel_reservation(&repo_id);
                }

                // Remove checkout manager if it exists
                if let Ok(mut checkout_managers) =
                    Self::handle_mutex_poison(self.checkout_managers.lock())
                {
                    if let Some(checkout_state) = checkout_managers.remove(scanner_id) {
                        // Perform cleanup on the checkout manager
                        if let Ok(mut manager) =
                            Self::handle_mutex_poison(checkout_state.manager.lock())
                        {
                            if !manager.keep_files {
                                if let Err(e) = manager.cleanup_all() {
                                    log::warn!(
                                        "Failed to cleanup checkout files for scanner '{}': {}",
                                        scanner_id,
                                        e
                                    );
                                }
                            }
                        } else {
                            log::error!("Failed to acquire checkout manager lock during cleanup for scanner '{}'", scanner_id);
                        }
                    }
                } else {
                    log::error!("Failed to acquire checkout_managers lock during scanner cleanup");
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
            let scanner_tasks =
                Self::handle_mutex_poison(self._scanner_tasks.lock()).map_err(|e| {
                    ScanError::Repository {
                        message: format!("Failed to access scanner tasks: {}", e),
                    }
                })?;

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
        Self::handle_mutex_poison(self._scanner_tasks.lock())
            .map(|tasks| tasks.len())
            .unwrap_or_else(|e| {
                log::error!("Failed to get scanner count: {}", e);
                0
            })
    }

    // ===== Checkout Management Methods =====

    /// Cleanup all checkouts when shutting down
    pub fn cleanup_all_checkouts(&self) {
        // Collect managers to clean up while holding the lock, then release lock before cleanup
        let managers_to_cleanup = {
            match Self::handle_mutex_poison(self.checkout_managers.lock()) {
                Ok(mut checkout_managers) => checkout_managers.drain().collect::<Vec<_>>(),
                Err(e) => {
                    log::error!(
                        "Failed to acquire checkout_managers lock for cleanup: {}",
                        e
                    );
                    return;
                }
            }
        };

        // Now cleanup each manager without holding the lock
        for (scanner_id, state) in managers_to_cleanup {
            match Self::handle_mutex_poison(state.manager.lock()) {
                Ok(mut manager) => {
                    if !manager.keep_files {
                        if let Err(e) = manager.cleanup_all() {
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
                Err(e) => {
                    log::error!(
                        "Failed to acquire manager lock for scanner '{}': {}",
                        scanner_id,
                        e
                    );
                }
            }
        }

        // Clear plugin mappings
        if let Ok(mut plugin_to_scanners) =
            Self::handle_mutex_poison(self.plugin_to_scanners.lock())
        {
            plugin_to_scanners.clear();
        } else {
            log::error!("Failed to clear plugin mappings during cleanup");
        }

        // Mark cleanup as done
        if let Ok(mut cleanup_done) = Self::handle_mutex_poison(self.cleanup_done.lock()) {
            *cleanup_done = true;
        }
    }

    /// Cleanup all checkouts with bounded blocking time
    ///
    /// Balances cleanup guarantees with performance by using timeouts.
    /// This prevents Drop from hanging indefinitely on slow I/O while still
    /// ensuring cleanup attempts are made.
    fn cleanup_all_checkouts_with_timeout(&self, timeout: Duration) {
        // Collect managers to clean up while holding the lock, then release lock before cleanup
        let managers_to_cleanup = {
            match Self::handle_mutex_poison(self.checkout_managers.lock()) {
                Ok(mut checkout_managers) => checkout_managers.drain().collect::<Vec<_>>(),
                Err(e) => {
                    log::error!(
                        "Failed to acquire checkout_managers lock for timeout cleanup: {}",
                        e
                    );
                    return;
                }
            }
        };

        // Clear plugin mappings immediately (fast operation)
        if let Ok(mut plugin_to_scanners) =
            Self::handle_mutex_poison(self.plugin_to_scanners.lock())
        {
            plugin_to_scanners.clear();
        } else {
            log::error!("Failed to clear plugin mappings during timeout cleanup");
        }

        // Mark cleanup as done immediately (fast operation)
        if let Ok(mut cleanup_done) = Self::handle_mutex_poison(self.cleanup_done.lock()) {
            *cleanup_done = true;
        } else {
            log::error!("Failed to mark cleanup as done during timeout cleanup");
        }

        // Perform file cleanup with overall timeout
        if !managers_to_cleanup.is_empty() {
            let cleanup_start = Instant::now();
            let mut completed_count = 0;

            for (scanner_id, state) in managers_to_cleanup.iter() {
                // Check if we've exceeded our overall timeout budget
                if cleanup_start.elapsed() >= timeout {
                    log::warn!(
                        "Cleanup timeout exceeded after {}ms; {} of {} managers cleaned up",
                        timeout.as_millis(),
                        completed_count,
                        managers_to_cleanup.len()
                    );
                    break;
                }

                match Self::handle_mutex_poison(state.manager.lock()) {
                    Ok(mut manager) => {
                        if !manager.keep_files {
                            let per_manager_start = Instant::now();

                            if let Err(e) = manager.cleanup_all() {
                                log::warn!(
                                    "Timeout cleanup failed for scanner '{}': {}",
                                    scanner_id,
                                    e
                                );
                            } else {
                                completed_count += 1;
                                let cleanup_duration = per_manager_start.elapsed();
                                if cleanup_duration.as_millis() > 50 {
                                    log::debug!(
                                        "Cleanup for scanner '{}' took {}ms",
                                        scanner_id,
                                        cleanup_duration.as_millis()
                                    );
                                }
                            }
                        } else {
                            completed_count += 1; // Still count it as completed
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to acquire manager lock for timeout cleanup of scanner '{}': {}", scanner_id, e);
                    }
                }
            }

            let total_duration = cleanup_start.elapsed();
            if total_duration.as_millis() > 100 {
                log::debug!(
                    "Timeout cleanup completed {} of {} managers in {}ms",
                    completed_count,
                    managers_to_cleanup.len(),
                    total_duration.as_millis()
                );
            }
        }
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
        self.cleanup_all_checkouts();
    }
}

impl Drop for ScannerManager {
    fn drop(&mut self) {
        // Prevent double cleanup by checking the flag
        let cleanup_already_done = match Self::handle_mutex_poison(self.cleanup_done.lock()) {
            Ok(guard) => *guard,
            Err(e) => {
                log::error!("Mutex poisoned during drop cleanup check: {}", e);
                log::warn!("Proceeding with cleanup to avoid resource leaks");
                false // Proceed with cleanup when in doubt to avoid resource leaks
            }
        };

        if !cleanup_already_done {
            // Get active checkout count for logging
            let active_count = Self::handle_mutex_poison(self.checkout_managers.lock())
                .map(|managers| managers.len())
                .unwrap_or_else(|e| {
                    log::error!("Failed to get checkout manager count during drop: {}", e);
                    0
                });

            if active_count > 0 {
                log::warn!(
                    "ScannerManager Drop: {} checkouts not cleaned up by shutdown coordinator - performing emergency cleanup with timeout",
                    active_count
                );

                // Emergency cleanup with strict timeout as safety net
                // This should rarely be needed
                // if shutdown coordinator works properly
                self.cleanup_all_checkouts_with_timeout(Self::EMERGENCY_CLEANUP_TIMEOUT);
            }
        } else {
            log::trace!("ScannerManager drop: cleanup already completed, skipping");
        }
    }
}
