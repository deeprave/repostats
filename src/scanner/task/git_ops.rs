//! Scanner Task Git Operations
//!
//! Git-related operations including repository access, commit scanning, and content reconstruction.

use crate::core::pattern_parser::AuthorPatternMatcher;
use crate::core::query::QueryParams;
use crate::notifications::api::ScanEventType;
use crate::scanner::error::{ScanError, ScanResult};
use crate::scanner::types::{
    ChangeType, CommitInfo, FileChangeData, RepositoryData, ScanMessage, ScanStats,
};
use gix;
use log;
use std::time::SystemTime;

use super::core::ScannerTask;

impl ScannerTask {
    /// Convert git time to SystemTime (helper to avoid duplication)
    fn git_time_to_system_time(time: &gix::date::Time) -> SystemTime {
        if time.seconds >= 0 {
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(time.seconds as u64)
        } else {
            let abs_seconds = (-time.seconds) as u64;
            SystemTime::UNIX_EPOCH - std::time::Duration::from_secs(abs_seconds)
        }
    }
    /// Extract repository metadata from the git repository
    pub async fn extract_repository_data(
        &self,
        query_params: Option<&QueryParams>,
        repository: &gix::Repository,
    ) -> ScanResult<RepositoryData> {
        let repository_path = self.repository_path().to_string();

        // Extract data from repository synchronously since we have it already
        let mut builder = RepositoryData::builder()
            .with_repository(repository_path)
            .with_repository_info(repository);

        // Add query parameters if provided
        if let Some(query) = query_params {
            builder = builder.with_query(query);
        }

        builder
            .build()
            .map_err(|e| ScanError::Repository { message: e })
    }

    /// Scan commits and publish messages incrementally to avoid memory buildup
    pub async fn scan_commits_and_publish_incrementally(&self) -> ScanResult<()> {
        self.scan_commits_and_publish_incrementally_with_query(None)
            .await
    }

    /// Scan commits with query parameters and publish messages with per-commit batching
    pub async fn scan_commits_and_publish_incrementally_with_query(
        &self,
        query_params: Option<&QueryParams>,
    ) -> ScanResult<()> {
        // Better separation: git_ops only handles domain objects, queue_ops handles serialization
        self.scan_commits_with_query(query_params, |msg| async move {
            // Just pass the domain object - no serialization in git_ops
            self.publish_message(msg).await
        })
        .await
    }

    /// Scan commits in the repository with query parameters using streaming callback pattern
    pub async fn scan_commits_with_query<F, Fut>(
        &self,
        query_params: Option<&QueryParams>,
        mut message_handler: F,
    ) -> ScanResult<()>
    where
        F: FnMut(ScanMessage) -> Fut,
        Fut: std::future::Future<Output = ScanResult<()>>,
    {
        // Publish scanner started event
        log::trace!("Publishing scanner started event");
        self.publish_scanner_event(
            ScanEventType::Started,
            Some("Starting repository scan".to_string()),
        )
        .await?;

        // Use the repository directly - it's already opened
        let repo = self.repository();

        // Start timing the actual scanning work
        let scan_start_time = SystemTime::now();

        // Initialize statistics tracking
        let mut total_files_changed = 0;
        let mut total_insertions = 0;
        let mut total_deletions = 0;

        // Create repository data for the message
        let mut builder = crate::scanner::types::RepositoryData::builder()
            .with_repository(self.repository_path())
            .with_repository_info(repo);

        if let Some(params) = query_params {
            builder = builder.with_query(params);
        }

        let repository_data = builder.build().map_err(|e| ScanError::Configuration {
            message: format!("Failed to build repository data: {}", e),
        })?;

        message_handler(ScanMessage::RepositoryData {
            scanner_id: self.scanner_id().to_string(),
            timestamp: SystemTime::now(),
            repository_data,
        })
        .await?;

        // Process commits directly into messages to avoid memory duplication

        // Pre-compile author patterns for performance
        let author_matcher = if let Some(ref params) = query_params {
            if !params.authors.include.is_empty() || !params.authors.exclude.is_empty() {
                Some(
                    AuthorPatternMatcher::new(&params.authors.include, &params.authors.exclude)
                        .map_err(|e| ScanError::Configuration {
                            message: format!("Invalid author filter pattern: {}", e),
                        })?,
                )
            } else {
                None
            }
        } else {
            None
        };

        // Determine starting point based on git_ref parameter
        let start_ref = if let Some(ref params) = query_params {
            if let Some(ref git_ref) = params.git_ref {
                git_ref.as_str()
            } else {
                "HEAD"
            }
        } else {
            "HEAD"
        };

        // Resolve and create commit walk from the starting reference
        let start_commit = if start_ref != "HEAD" {
            // Resolve the starting reference and handle annotated tags
            let start_commit_id =
                repo.rev_parse_single(start_ref)
                    .map_err(|e| ScanError::Repository {
                        message: format!("Failed to resolve reference '{}': {}", start_ref, e),
                    })?;

            let start_object =
                repo.find_object(start_commit_id)
                    .map_err(|e| ScanError::Repository {
                        message: format!("Failed to get object from ref '{}': {}", start_ref, e),
                    })?;

            // Peel the object to handle annotated tags and get underlying commits
            start_object
                .peel_to_kind(gix::object::Kind::Commit)
                .map_err(|e| ScanError::Repository {
                    message: format!(
                        "Failed to resolve '{}' to a commit (may be tag, tree, or blob): {}",
                        start_ref, e
                    ),
                })?
                .try_into_commit()
                .map_err(|_| ScanError::Repository {
                    message: format!(
                        "Reference '{}' does not ultimately point to a commit",
                        start_ref
                    ),
                })?
        } else {
            match repo.head_commit() {
                Ok(commit) => commit,
                Err(e) => {
                    // Check if this is an empty repository (no commits)
                    if e.to_string().contains("does not have any commits")
                        || e.to_string().contains("unborn")
                    {
                        // Empty repository - send completion with zero stats
                        message_handler(ScanMessage::ScanCompleted {
                            scanner_id: self.scanner_id().to_string(),
                            timestamp: SystemTime::now(),
                            stats: ScanStats {
                                total_commits: 0,
                                total_files_changed: 0,
                                total_insertions: 0,
                                total_deletions: 0,
                                scan_duration: scan_start_time
                                    .elapsed()
                                    .unwrap_or(std::time::Duration::from_millis(0)),
                            },
                        })
                        .await?;

                        // Publish scanner completed event
                        self.publish_scanner_event(
                            ScanEventType::Completed,
                            Some("Empty repository scan completed".to_string()),
                        )
                        .await?;

                        return Ok(());
                    }
                    return Err(ScanError::Repository {
                        message: format!("Failed to get HEAD commit: {}", e),
                    });
                }
            }
        };

        let walk = start_commit
            .ancestors()
            .all()
            .map_err(|e| ScanError::Repository {
                message: format!("Failed to create commit walk from '{}': {}", start_ref, e),
            })?;

        // Process commits with filtering
        let mut commit_count = 0;
        let max_commits = query_params.as_ref().and_then(|p| p.max_commits);

        for commit_result in walk {
            // Check if we've reached max_commits limit
            if let Some(max) = max_commits {
                if commit_count >= max {
                    break;
                }
            }

            let commit_info = commit_result.map_err(|e| ScanError::Repository {
                message: format!("Failed to get commit during traversal: {}", e),
            })?;

            let commit = commit_info.object().map_err(|e| ScanError::Repository {
                message: format!("Failed to get commit object: {}", e),
            })?;

            // Get commit metadata
            let author = commit.author().map_err(|e| ScanError::Repository {
                message: format!("Failed to get author: {}", e),
            })?;
            let committer = commit.committer().map_err(|e| ScanError::Repository {
                message: format!("Failed to get committer: {}", e),
            })?;
            let time = commit.time().map_err(|e| ScanError::Repository {
                message: format!("Failed to get time: {}", e),
            })?;
            let message = commit.message().map_err(|e| ScanError::Repository {
                message: format!("Failed to get message: {}", e),
            })?;

            // Apply author filtering using pre-compiled matcher
            if let Some(ref matcher) = author_matcher {
                if !matcher.matches(&author.name.to_string(), &author.email.to_string()) {
                    continue; // Skip this commit
                }
            }

            // Apply date range filtering
            if let Some(ref params) = query_params {
                if let Some(ref date_range) = params.date_range {
                    let commit_time = Self::git_time_to_system_time(&time);

                    if !date_range.contains(commit_time) {
                        continue; // Skip this commit
                    }
                }
            }

            // Apply merge commit filtering
            if let Some(ref params) = query_params {
                if !params.should_include_merge_commits() {
                    // Check if this is a merge commit (has more than one parent)
                    let parent_count = commit.parent_ids().count();
                    if parent_count > 1 {
                        continue; // Skip this merge commit
                    }
                }
            }

            let hash_string = commit.id().to_string();
            let short_hash = hash_string.get(..8).unwrap_or(&hash_string).to_string();

            let commit_info = CommitInfo {
                hash: hash_string,
                short_hash,
                author_name: author.name.to_string(),
                author_email: author.email.to_string(),
                committer_name: committer.name.to_string(),
                committer_email: committer.email.to_string(),
                timestamp: Self::git_time_to_system_time(&time),
                message: message
                    .body()
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| message.summary().to_string()),
                parent_hashes: commit.parent_ids().map(|id| id.to_string()).collect(),
                insertions: 0, // TODO: Implement diff parsing in future issue (RS-XX)
                deletions: 0,  // TODO: Implement diff parsing in future issue (RS-XX)
            };

            message_handler(ScanMessage::CommitData {
                scanner_id: self.scanner_id().to_string(),
                timestamp: SystemTime::now(),
                commit_info,
            })
            .await?;

            // Process file changes if required
            if self.requirements().requires_file_changes() {
                let file_changes = self.analyze_commit_diff(&commit).await?;

                // Accumulate statistics from file changes
                let mut unique_files = std::collections::HashSet::new();
                for file_change_msg in file_changes {
                    if let ScanMessage::FileChange {
                        file_path,
                        change_data,
                        ..
                    } = &file_change_msg
                    {
                        unique_files.insert(file_path.clone());
                        total_insertions += change_data.insertions;
                        total_deletions += change_data.deletions;

                        // Forward the file change message
                        message_handler(file_change_msg).await?;
                    }
                }
                total_files_changed += unique_files.len();
            }

            commit_count += 1;
        }

        // Add scan completed message
        message_handler(ScanMessage::ScanCompleted {
            scanner_id: self.scanner_id().to_string(),
            timestamp: SystemTime::now(),
            stats: ScanStats {
                total_commits: commit_count,
                total_files_changed,
                total_insertions,
                total_deletions,
                scan_duration: scan_start_time
                    .elapsed()
                    .unwrap_or(std::time::Duration::from_millis(0)),
            },
        })
        .await?;

        // Publish scanner completed event
        self.publish_scanner_event(
            ScanEventType::Completed,
            Some("Repository scan completed successfully".to_string()),
        )
        .await?;

        Ok(())
    }

    /// Resolve start point (commit SHA, branch name, tag name) to full commit SHA
    pub async fn resolve_start_point(&self, start_point: &str) -> ScanResult<String> {
        let repository_path = self.repository_path().to_string();
        let start_point = start_point.to_string();

        // Use spawn_blocking for potentially blocking git operations
        tokio::task::spawn_blocking(move || {
            let repo = gix::open(&repository_path).map_err(|e| ScanError::Repository {
                message: format!("Failed to open repository: {}", e),
            })?;

            // Try to resolve the reference
            let resolved =
                repo.rev_parse_single(start_point.as_str())
                    .map_err(|e| ScanError::Repository {
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

    /// Read current file content from working directory (placeholder for historical reconstruction)
    ///
    /// TODO: RS-XX - Replace with actual Git historical file reconstruction
    ///
    /// This is a placeholder implementation that reads from the working directory, not Git history.
    /// The commit_sha parameter is validated but not used for actual file retrieval.
    /// Real implementation should read file content from the specified commit.
    pub async fn read_current_file_content(
        &self,
        file_path: &str,
        _commit_sha: &str, // Underscore prefix indicates intentionally unused parameter
    ) -> ScanResult<String> {
        let repository_path = self.repository_path().to_string();
        let file_path = file_path.to_string();
        let commit_sha = _commit_sha.to_string();

        // Use spawn_blocking for potentially blocking git operations
        tokio::task::spawn_blocking(move || {
            let repo = gix::open(&repository_path).map_err(|e| ScanError::Repository {
                message: format!("Failed to open repository: {}", e),
            })?;

            // Validate that the commit exists
            repo.rev_parse_single(commit_sha.as_str())
                .map_err(|e| ScanError::Repository {
                    message: format!("Failed to resolve commit '{}': {}", commit_sha, e),
                })?;

            // PLACEHOLDER: Read from working directory, not Git history
            // TODO: RS-XX - Implement actual historical file reconstruction from commit
            let file_full_path = std::path::Path::new(&repository_path).join(&file_path);

            if !file_full_path.exists() {
                return Err(ScanError::Repository {
                    message: format!("File '{}' not found in commit '{}'", file_path, commit_sha),
                });
            }

            let content =
                std::fs::read_to_string(&file_full_path).map_err(|e| ScanError::Repository {
                    message: format!("Failed to read file '{}': {}", file_path, e),
                })?;

            Ok(content)
        })
        .await
        .map_err(|e| ScanError::Io {
            message: format!("Failed to execute git operation: {}", e),
        })?
    }

    /// PLACEHOLDER: Generate minimal file change data for TDD scaffolding
    ///
    /// TODO: RS-XX - Replace with actual Git diff parsing implementation
    ///
    /// This is a placeholder implementation that returns minimal valid data structures
    /// to support TDD test development. It does NOT perform actual Git diff analysis.
    /// Real implementation should parse commit diffs using gix tree comparison.
    pub async fn analyze_commit_diff(
        &self,
        commit: &gix::Commit<'_>,
    ) -> ScanResult<Vec<ScanMessage>> {
        log::warn!(
            "analyze_commit_diff is using placeholder implementation (commit: {})",
            commit.id().to_hex_with_len(8)
        );

        let commit_info = self.extract_commit_info(commit)?;

        // Return minimal placeholder file change for TDD compatibility
        let placeholder_change = FileChangeData {
            change_type: ChangeType::Modified,
            old_path: None,
            new_path: "placeholder.rs".to_string(),
            insertions: 0,
            deletions: 0,
            is_binary: false,
            checkout_path: None,
        };

        Ok(vec![ScanMessage::FileChange {
            scanner_id: self.scanner_id().to_string(),
            file_path: "placeholder.rs".to_string(),
            change_data: placeholder_change,
            commit_context: commit_info,
            timestamp: std::time::SystemTime::now(),
        }])
    }

    /// Extract commit information from a Git commit object
    fn extract_commit_info(&self, commit: &gix::Commit<'_>) -> ScanResult<CommitInfo> {
        let commit_id = commit.id();
        let author = commit.author().map_err(|e| ScanError::Repository {
            message: format!("Failed to get commit author: {}", e),
        })?;
        let time = commit.time().map_err(|e| ScanError::Repository {
            message: format!("Failed to get commit time: {}", e),
        })?;
        let message = commit.message().map_err(|e| ScanError::Repository {
            message: format!("Failed to get commit message: {}", e),
        })?;

        Ok(CommitInfo {
            hash: commit_id.to_hex_with_len(40).to_string(),
            short_hash: commit_id.to_hex_with_len(8).to_string(),
            author_name: author.name.to_string(),
            author_email: author.email.to_string(),
            committer_name: author.name.to_string(),
            committer_email: author.email.to_string(),
            timestamp: Self::git_time_to_system_time(&time),
            message: message.title.to_string(),
            parent_hashes: commit
                .parent_ids()
                .map(|id| id.to_hex_with_len(40).to_string())
                .collect(),
            insertions: 0,
            deletions: 0,
        })
    }

    /// Scan file changes from commits based on requirements
    async fn scan_file_changes(
        &self,
        repo: &gix::Repository,
        query_params: Option<&QueryParams>,
    ) -> ScanResult<Vec<ScanMessage>> {
        let mut file_change_messages = Vec::new();

        // Check query parameters for early exit conditions
        if let Some(params) = query_params {
            if let Some(max_commits) = params.max_commits {
                if max_commits == 0 {
                    return Ok(Vec::new()); // Early exit for zero commit limit
                }
            }
        }

        // Get HEAD commit for basic file change scanning
        let mut head = repo.head().map_err(|e| ScanError::Repository {
            message: format!("Failed to get HEAD: {}", e),
        })?;
        let commit = head
            .peel_to_commit_in_place()
            .map_err(|e| ScanError::Repository {
                message: format!("Failed to get commit: {}", e),
            })?;

        // Extract commit information for context
        let hash_string = commit.id().to_string();
        let short_hash = hash_string.get(..8).unwrap_or(&hash_string).to_string();
        let author = commit.author().map_err(|e| ScanError::Repository {
            message: format!("Failed to get author: {}", e),
        })?;
        let committer = commit.committer().map_err(|e| ScanError::Repository {
            message: format!("Failed to get committer: {}", e),
        })?;
        let time = commit.time().map_err(|e| ScanError::Repository {
            message: format!("Failed to get time: {}", e),
        })?;
        let message = commit.message().map_err(|e| ScanError::Repository {
            message: format!("Failed to get message: {}", e),
        })?;

        let commit_info = CommitInfo {
            hash: hash_string,
            short_hash,
            author_name: author.name.to_string(),
            author_email: author.email.to_string(),
            committer_name: committer.name.to_string(),
            committer_email: committer.email.to_string(),
            timestamp: Self::git_time_to_system_time(&time),
            message: message
                .body()
                .map(|b| b.to_string())
                .unwrap_or_else(|| message.summary().to_string()),
            parent_hashes: commit.parent_ids().map(|id| id.to_string()).collect(),
            insertions: 0,
            deletions: 0,
        };

        // For basic implementation, create a sample file change
        // In a full implementation, this would analyze the commit diff
        let file_change = FileChangeData {
            change_type: ChangeType::Modified,
            old_path: None,
            new_path: "sample-file.txt".to_string(),
            insertions: 10,
            deletions: 5,
            is_binary: false,
            checkout_path: None,
        };

        file_change_messages.push(ScanMessage::FileChange {
            scanner_id: self.scanner_id().to_string(),
            file_path: "sample-file.txt".to_string(),
            change_data: file_change,
            commit_context: commit_info,
            timestamp: SystemTime::now(),
        });

        Ok(file_change_messages)
    }
}

// Tests are now organized in the tests module for better maintainability
#[cfg(test)]
mod tests {
    // Removed unused import: super::super::tests::*
}
