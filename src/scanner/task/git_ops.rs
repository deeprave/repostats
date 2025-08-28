//! Scanner Task Git Operations
//!
//! Git-related operations including repository access, commit scanning, and content reconstruction.

use crate::core::query::QueryParams;
use crate::notifications::event::ScanEventType;
use crate::scanner::error::{ScanError, ScanResult};
use crate::scanner::types::{CommitInfo, RepositoryData, ScanMessage, ScanStats};
use gix;
use std::path::Path;
use std::time::SystemTime;

use super::core::ScannerTask;

impl ScannerTask {
    /// Extract repository metadata from the git repository
    pub async fn extract_repository_data(
        &self,
        query_params: Option<&QueryParams>,
    ) -> ScanResult<RepositoryData> {
        let repo = self.open_repository().await?;
        let repository_path = self.repository_path().to_string();

        // Clone query_params for move into spawn_blocking
        let query_params = query_params.cloned();

        // Use spawn_blocking for potentially blocking gix operations
        tokio::task::spawn_blocking(move || {
            // Use builder pattern to create RepositoryData
            let mut builder = RepositoryData::builder()
                .with_repository(repository_path)
                .with_repository_info(&repo);

            // Add query parameters if provided
            if let Some(ref query) = query_params {
                builder = builder.with_query(query);
            }

            builder.build().map_err(|e| {
                Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))
                    as Box<dyn std::error::Error + Send + Sync>
            })
        })
        .await
        .map_err(|e| ScanError::Io {
            message: format!("Task execution failed: {}", e),
        })?
        .map_err(
            |e: Box<dyn std::error::Error + Send + Sync>| ScanError::Repository {
                message: format!("Failed to extract repository data: {}", e),
            },
        )
    }

    /// Open the git repository using gix
    pub async fn open_repository(&self) -> ScanResult<gix::Repository> {
        // Check if we have a cached repository instance
        if let Some(repo_arc) = self.repository() {
            // Convert ThreadSafeRepository to Repository for this thread
            return Ok(repo_arc.to_thread_local());
        }

        // Fallback: open repository if not cached (shouldn't happen with new create_scanner)
        if !self.is_remote() {
            // It's a local path
            let path = Path::new(self.repository_path());

            // Use spawn_blocking for potentially blocking gix operations
            let repo_path = path.to_path_buf();
            let repo = tokio::task::spawn_blocking(move || gix::discover(&repo_path))
                .await
                .map_err(|e| ScanError::Io {
                    message: format!("Task execution failed: {}", e),
                })?
                .map_err(|e| ScanError::Repository {
                    message: format!("Failed to open repository '{}': {}", path.display(), e),
                })?;

            Ok(repo)
        } else {
            // Remote repository - not yet supported
            Err(ScanError::Configuration {
                message: "Remote repository support not yet implemented".to_string(),
            })
        }
    }

    /// Scan commits in the repository and generate scan messages
    pub async fn scan_commits(&self) -> ScanResult<Vec<ScanMessage>> {
        self.scan_commits_with_query(None).await
    }

    /// Scan commits in the repository with query parameters
    pub async fn scan_commits_with_query(
        &self,
        query_params: Option<&QueryParams>,
    ) -> ScanResult<Vec<ScanMessage>> {
        // Publish scanner started event
        self.publish_scanner_event(
            ScanEventType::Started,
            Some("Starting repository scan".to_string()),
        )
        .await?;

        let mut messages = Vec::new();

        // FIRST: Extract and add repository data as the very first message
        let repository_data = match self.extract_repository_data(query_params).await {
            Ok(data) => data,
            Err(e) => {
                let error_msg = format!("Failed to extract repository data: {}", e);
                self.publish_scanner_event(ScanEventType::Error, Some(error_msg.clone()))
                    .await
                    .ok(); // Don't fail on event error
                return Err(e);
            }
        };

        messages.push(ScanMessage::RepositoryData {
            scanner_id: self.scanner_id().to_string(),
            repository_data,
            timestamp: SystemTime::now(),
        });

        // Open the repository - if this fails, publish error event
        let repo = match self.open_repository().await {
            Ok(repo) => repo,
            Err(e) => {
                let error_msg = format!("Failed to open repository: {}", e);
                self.publish_scanner_event(ScanEventType::Error, Some(error_msg.clone()))
                    .await
                    .ok(); // Don't fail on event error
                return Err(e);
            }
        };

        // Add scan started message
        messages.push(ScanMessage::ScanStarted {
            scanner_id: self.scanner_id().to_string(),
            repository_path: self.repository_path().to_string(),
            timestamp: SystemTime::now(),
        });

        // Use spawn_blocking for potentially blocking git operations
        let scanner_id = self.scanner_id().to_string();

        let commit_messages = tokio::task::spawn_blocking(move || {
            let mut result_messages = Vec::new();

            // Get HEAD reference and traverse commits
            let mut head = repo.head()?;
            let commit = head.peel_to_commit_in_place()?;

            // Basic commit traversal - just get the HEAD commit for now
            let author = commit.author()?;
            let committer = commit.committer()?;
            let time = commit.time()?;
            let message = commit.message()?;

            let commit_info = CommitInfo {
                hash: commit.id().to_string(),
                short_hash: commit.id().to_string()[..8].to_string(),
                author_name: author.name.to_string(),
                author_email: author.email.to_string(),
                committer_name: committer.name.to_string(),
                committer_email: committer.email.to_string(),
                timestamp: SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(time.seconds as u64),
                message: message
                    .body()
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| message.summary().to_string()),
                parent_hashes: commit.parent_ids().map(|id| id.to_string()).collect(),
                insertions: 0, // Will be implemented with diff parsing
                deletions: 0,  // Will be implemented with diff parsing
            };

            result_messages.push(ScanMessage::CommitData {
                scanner_id: scanner_id.clone(),
                commit_info,
                timestamp: SystemTime::now(),
            });

            Ok::<Vec<ScanMessage>, Box<dyn std::error::Error + Send + Sync>>(result_messages)
        })
        .await
        .map_err(|e| ScanError::Io {
            message: format!("Task execution failed: {}", e),
        })?
        .map_err(|e| ScanError::Repository {
            message: format!("Failed to scan commits: {}", e),
        })?;

        messages.extend(commit_messages);

        // Add scan completed message
        messages.push(ScanMessage::ScanCompleted {
            scanner_id: self.scanner_id().to_string(),
            repository_path: self.repository_path().to_string(),
            stats: ScanStats {
                total_commits: 1, // Basic implementation with just HEAD
                total_files_changed: 0,
                total_insertions: 0,
                total_deletions: 0,
                scan_duration: std::time::Duration::from_millis(0),
            },
            timestamp: SystemTime::now(),
        });

        // Publish scanner completed event
        self.publish_scanner_event(
            ScanEventType::Completed,
            Some("Repository scan completed successfully".to_string()),
        )
        .await?;

        Ok(messages)
    }

    /// Resolve start point (commit SHA, branch name, tag name) to full commit SHA
    pub async fn resolve_start_point(&self, start_point: &str) -> ScanResult<String> {
        let repository_path = self.repository_path().to_string();
        let start_point = start_point.to_string();

        // Use spawn_blocking for potentially blocking git operations
        tokio::task::spawn_blocking(move || {
            let repo = gix::open(&repository_path).map_err(|e| ScanError::Git {
                message: format!("Failed to open repository: {}", e),
            })?;

            // Try to resolve the reference
            let resolved =
                repo.rev_parse_single(start_point.as_str())
                    .map_err(|e| ScanError::Git {
                        message: format!("Failed to resolve reference '{}': {}", start_point, e),
                    })?;

            // Get the commit SHA
            let commit_id = resolved.to_hex_with_len(40).to_string();
            Ok(commit_id)
        })
        .await
        .map_err(|e| ScanError::Io {
            message: format!("Failed to execute git operation: {}", e),
        })?
    }

    /// Reconstruct file content at a specific commit using git operations
    pub async fn reconstruct_file_content(
        &self,
        file_path: &str,
        commit_sha: &str,
    ) -> ScanResult<String> {
        let repository_path = self.repository_path().to_string();
        let file_path = file_path.to_string();
        let commit_sha = commit_sha.to_string();

        // Use spawn_blocking for potentially blocking git operations
        tokio::task::spawn_blocking(move || {
            let repo = gix::open(&repository_path).map_err(|e| ScanError::Git {
                message: format!("Failed to open repository: {}", e),
            })?;

            // Validate that the commit exists
            repo.rev_parse_single(commit_sha.as_str())
                .map_err(|e| ScanError::Git {
                    message: format!("Failed to resolve commit '{}': {}", commit_sha, e),
                })?;

            // For Phase 6 initial implementation: read file from working directory
            // This demonstrates the API and basic functionality
            // Full historical reconstruction will be implemented in later phases
            let file_full_path = std::path::Path::new(&repository_path).join(&file_path);

            if !file_full_path.exists() {
                return Err(ScanError::Git {
                    message: format!("File '{}' not found in commit '{}'", file_path, commit_sha),
                });
            }

            let content = std::fs::read_to_string(&file_full_path).map_err(|e| ScanError::Git {
                message: format!("Failed to read file '{}': {}", file_path, e),
            })?;

            Ok(content)
        })
        .await
        .map_err(|e| ScanError::Io {
            message: format!("Failed to execute git operation: {}", e),
        })?
    }
}
