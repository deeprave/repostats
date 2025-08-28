//! Scanner Manager
//!
//! Central coordination component for managing multiple repository scanner tasks,
//! each with unique SHA256-based identification to prevent duplicate scanning.

use crate::scanner::error::{ScanError, ScanResult};
use crate::scanner::task::ScannerTask;
use gix_url;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
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

/// Central scanner manager for coordinating multiple repository scanner tasks
pub struct ScannerManager {
    /// Active scanner tasks by repository hash
    _scanner_tasks: HashMap<String, String>, // hash -> repository path
    /// Repository states to prevent duplicate scanners with reservation system
    repo_states: Mutex<HashMap<String, RepoState>>,
}

impl ScannerManager {
    /// Create a new ScannerManager instance
    pub fn new() -> Self {
        Self {
            _scanner_tasks: HashMap::new(),
            repo_states: Mutex::new(HashMap::new()),
        }
    }

    /// Redact sensitive information from repository paths for secure logging
    fn redact_repo_path(&self, path: &str) -> String {
        // Handle Windows UNC paths first (e.g., \\server\share\path\to\repo)
        if path.starts_with(r"\\") {
            let parts: Vec<&str> = path[2..]
                .split(['\\', '/'].as_ref())
                .filter(|s| !s.is_empty())
                .collect();
            return if parts.len() >= 2 {
                format!(r"\\{}\{}\REDACTED", parts[0], parts[1])
            } else if parts.len() == 1 {
                format!(r"\\{}\REDACTED", parts[0])
            } else {
                r"\\REDACTED".to_string()
            };
        }

        // Handle Windows drive paths first (before URL parsing) - e.g., C:\path\to\repo
        if path.len() >= 3
            && path.chars().nth(1) == Some(':')
            && (path.chars().nth(2) == Some('\\') || path.chars().nth(2) == Some('/'))
        {
            let drive = &path[..3];
            return format!("{}REDACTED", drive);
        }

        // Try to parse as a git URL using gix-url only if it looks like a URL
        // (contains scheme or starts with protocol patterns)
        if path.contains("://") || path.starts_with("file:") {
            if let Ok(url) = gix_url::Url::from_bytes(path.as_bytes().into()) {
                // Build redacted URL with scheme and host only
                let scheme = url.scheme.as_str();
                let host = url
                    .host()
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| "unknown-host".to_string());

                // Handle different schemes appropriately
                return match url.scheme {
                    gix_url::Scheme::Http | gix_url::Scheme::Https => {
                        format!("{}://{}/REDACTED", scheme, host)
                    }
                    gix_url::Scheme::Ssh => {
                        format!("ssh://{}/REDACTED", host)
                    }
                    gix_url::Scheme::Git => {
                        format!("git://{}/REDACTED", host)
                    }
                    gix_url::Scheme::File => "file://REDACTED".to_string(),
                    _ => {
                        format!("{}://REDACTED", scheme)
                    }
                };
            }
        }

        // Handle SSH URLs without explicit scheme (e.g., git@github.com:user/repo.git)
        // Only match if it has @ and : but doesn't look like a Windows path
        if path.contains('@') && path.contains(':') && !path.contains("://") {
            let after_at = path.split('@').last().unwrap_or(path);
            let host = after_at.split(':').next().unwrap_or(after_at);
            return format!("ssh://{}/REDACTED", host);
        }

        // Handle absolute paths by keeping root and redacting the rest
        if path.starts_with('/') {
            return "/REDACTED".to_string();
        }

        // For relative paths, show only the last component to avoid exposing directory structure
        if let Some(last_separator_pos) = path.rfind('/').or_else(|| path.rfind('\\')) {
            let filename = &path[last_separator_pos + 1..];
            if !filename.is_empty() {
                let separator = if path.contains('\\') && !path.contains('/') {
                    '\\'
                } else {
                    '/'
                };
                return format!("REDACTED{}{}", separator, filename);
            }
        }

        // Fallback: completely redact anything we don't recognize
        "REDACTED".to_string()
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
    pub fn validate_repository(
        &self,
        repository_path: &Path,
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
    pub fn generate_scanner_id(&self, repo_id: &str) -> ScanResult<String> {
        // Generate SHA256 hash of the unique repo ID
        let mut hasher = Sha256::new();
        hasher.update(repo_id.as_bytes());
        let hash_result = hasher.finalize();

        // Convert to hex string and create scanner ID with scan- prefix
        let hash_hex = format!("{:x}", hash_result);
        Ok(format!("scan-{}", hash_hex))
    }

    /// Create a scanner for a repository with queue integration
    pub async fn create_scanner(&self, repository_path: &str) -> ScanResult<ScannerTask> {
        // First normalize the path
        let normalized_path = self.normalise_repository_path(repository_path)?;

        // Validate the repository and get the gix::Repository instance
        let path = Path::new(&normalized_path);
        let (repo, _git_dir) = self.validate_repository(path)?;

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

        // Create scanner task with the repository directly
        let scanner_task =
            ScannerTask::new_with_repository(scanner_id, normalized_path.clone(), repo);

        // Create queue publisher to ensure queue is ready, with retries for transient errors
        let mut last_publisher_err = None;
        let mut publisher = None;
        let max_retries = 3;
        let retry_delay = Duration::from_millis(500);

        for attempt in 0..max_retries {
            match scanner_task.create_queue_publisher().await {
                Ok(pub_result) => {
                    publisher = Some(pub_result);
                    break;
                }
                Err(e) => {
                    last_publisher_err = Some(e);
                    if attempt < max_retries - 1 {
                        log::debug!(
                            "Queue publisher creation attempt {} failed for '{}', retrying in {}ms",
                            attempt + 1,
                            self.redact_repo_path(repository_path),
                            retry_delay.as_millis()
                        );
                        tokio::time::sleep(retry_delay).await;
                    }
                }
            }
        }

        let _publisher = match publisher {
            Some(p) => p,
            None => {
                // Cancel reservation on failure
                self.cancel_reservation(&repo_id);
                let e = last_publisher_err.unwrap();
                return Err(ScanError::Configuration {
                    message: format!(
                        "Failed to create queue publisher for '{}' after {} attempts: {}",
                        repository_path, max_retries, e
                    ),
                });
            }
        };

        // Create notification subscriber with retries for transient errors
        let mut last_subscriber_err = None;
        let mut subscriber = None;

        for attempt in 0..max_retries {
            match scanner_task.create_notification_subscriber().await {
                Ok(sub_result) => {
                    subscriber = Some(sub_result);
                    break;
                }
                Err(e) => {
                    last_subscriber_err = Some(e);
                    if attempt < max_retries - 1 {
                        log::debug!(
                            "Notification subscriber creation attempt {} failed for '{}', retrying in {}ms",
                            attempt + 1,
                            self.redact_repo_path(repository_path),
                            retry_delay.as_millis()
                        );
                        tokio::time::sleep(retry_delay).await;
                    }
                }
            }
        }

        let _subscriber = match subscriber {
            Some(s) => s,
            None => {
                // Cancel reservation on failure
                self.cancel_reservation(&repo_id);
                let e = last_subscriber_err.unwrap();
                return Err(ScanError::Configuration {
                    message: format!(
                        "Failed to create notification subscriber for '{}' after {} attempts: {}",
                        repository_path, max_retries, e
                    ),
                });
            }
        };

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

        log::debug!(
            "Scanner created successfully for repository: {} (Scanner: {})",
            self.redact_repo_path(repository_path),
            scanner_task.scanner_id()
        );
        Ok(scanner_task)
    }
}

// Tests are in manager_tests.rs
#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
