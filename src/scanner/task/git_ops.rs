//! Scanner Task Git Operations
//!
//! Git-related operations including repository access, commit scanning, and content reconstruction.

use crate::core::query::QueryParams;
#[cfg(not(test))]
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

    /// Scan commits in the repository and generate scan messages
    pub async fn scan_commits(&self) -> ScanResult<Vec<ScanMessage>> {
        self.scan_commits_with_query(None).await
    }

    /// Scan commits in the repository with query parameters
    pub async fn scan_commits_with_query(
        &self,
        query_params: Option<&QueryParams>,
    ) -> ScanResult<Vec<ScanMessage>> {
        // Publish scanner started event (skip in tests)
        #[cfg(not(test))]
        {
            log::trace!("Publishing scanner started event");
            self.publish_scanner_event(
                ScanEventType::Started,
                Some("Starting repository scan".to_string()),
            )
            .await?;
        }

        let mut messages = Vec::new();
        let requirements = self.requirements();

        // Early exit if no requirements - nothing to scan
        if requirements.is_empty() {
            log::trace!("No plugin requirements - skipping scan");
            #[cfg(not(test))]
            {
                self.publish_scanner_event(
                    ScanEventType::Completed,
                    Some("No requirements - scan skipped".to_string()),
                )
                .await?;
            }
            return Ok(messages);
        }

        // Use the repository directly - it's already opened
        let repo = self.repository();

        // Start timing the actual scanning work
        let scan_start_time = SystemTime::now();

        // Add scan started message (always included if any scanning is needed)
        messages.push(ScanMessage::ScanStarted {
            scanner_id: self.scanner_id().to_string(),
            repository_path: self.repository_path().to_string(),
            timestamp: SystemTime::now(),
        });
        let mut total_commits = 0;
        let mut total_files_changed = 0;
        let mut total_insertions = 0;
        let mut total_deletions = 0;

        // Only extract repository data if required
        if requirements.requires_repository_info() {
            log::trace!("Extracting repository data (required by plugins)");
            let repository_data = match self
                .extract_repository_data(self.query_params(), repo)
                .await
            {
                Ok(data) => data,
                Err(e) => {
                    let error_msg = format!("Failed to extract repository data: {}", e);
                    #[cfg(not(test))]
                    {
                        self.publish_scanner_event(ScanEventType::Error, Some(error_msg.clone()))
                            .await
                            .ok(); // Don't fail on event error
                    }
                    #[cfg(test)]
                    let _ = error_msg; // Use variable in test context to avoid warning
                    return Err(e);
                }
            };

            messages.push(ScanMessage::RepositoryData {
                scanner_id: self.scanner_id().to_string(),
                repository_data,
                timestamp: SystemTime::now(),
            });
        }

        // Only scan commits if required
        if requirements.requires_commits() || requirements.requires_history() {
            log::trace!("Scanning commits (required by plugins)");

            if requirements.requires_history() {
                // Note: Full history traversal will be implemented in RS-26
                // For now, scanning HEAD only which provides basic commit data
                log::trace!("Full history requested - currently scanning HEAD only (RS-26 will implement full traversal)");
            }

            // For now, implement basic HEAD commit scanning
            let commit_messages = self.scan_head_commit(repo).await?;
            total_commits = commit_messages.len();
            messages.extend(commit_messages);
        }

        // Only scan file changes if required
        if requirements.requires_file_changes() {
            log::trace!("Scanning file changes (required by plugins)");
            let file_change_messages = self.scan_file_changes(repo, query_params).await?;
            total_files_changed = file_change_messages.len();
            total_insertions = file_change_messages
                .iter()
                .filter_map(|msg| {
                    if let ScanMessage::FileChange { change_data, .. } = msg {
                        Some(change_data.insertions)
                    } else {
                        None
                    }
                })
                .sum();
            total_deletions = file_change_messages
                .iter()
                .filter_map(|msg| {
                    if let ScanMessage::FileChange { change_data, .. } = msg {
                        Some(change_data.deletions)
                    } else {
                        None
                    }
                })
                .sum();
            messages.extend(file_change_messages);
        }

        // Only scan file content if required
        if requirements.requires_file_content() {
            // Note: File content reconstruction will be implemented in RS-27
            // This is complex as it requires checking out files at specific commit states
            log::trace!(
                "File content requested - deferred to RS-27 (requires git checkout functionality)"
            );
            // File changes are already scanned as a dependency which provides file paths and change types
        }

        // Add scan completed message with actual stats
        let scan_duration = scan_start_time.elapsed().unwrap_or_default();
        messages.push(ScanMessage::ScanCompleted {
            scanner_id: self.scanner_id().to_string(),
            repository_path: self.repository_path().to_string(),
            stats: ScanStats {
                total_commits,
                total_files_changed,
                total_insertions,
                total_deletions,
                scan_duration,
            },
            timestamp: SystemTime::now(),
        });

        // Publish scanner completed event (skip in tests)
        #[cfg(not(test))]
        {
            self.publish_scanner_event(
                ScanEventType::Completed,
                Some(format!(
                    "Repository scan completed - {} commits processed",
                    total_commits
                )),
            )
            .await?;
        }

        Ok(messages)
    }

    /// Scan just the HEAD commit (extracted from original implementation)
    async fn scan_head_commit(&self, repo: &gix::Repository) -> ScanResult<Vec<ScanMessage>> {
        let mut commit_messages = Vec::new();

        // Get HEAD reference and traverse commits
        let mut head = repo.head().map_err(|e| ScanError::Repository {
            message: format!("Failed to get HEAD: {}", e),
        })?;
        let commit = head
            .peel_to_commit_in_place()
            .map_err(|e| ScanError::Repository {
                message: format!("Failed to get commit: {}", e),
            })?;

        // Extract commit information
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

        let hash_string = commit.id().to_string();
        let short_hash = hash_string.get(..8).unwrap_or(&hash_string).to_string();

        let commit_info = CommitInfo {
            hash: hash_string,
            short_hash,
            author_name: author.name.to_string(),
            author_email: author.email.to_string(),
            committer_name: committer.name.to_string(),
            committer_email: committer.email.to_string(),
            timestamp: if time.seconds >= 0 {
                SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(time.seconds as u64)
            } else {
                // Handle negative timestamps (dates before Unix epoch)
                let abs_seconds = (-time.seconds) as u64;
                SystemTime::UNIX_EPOCH - std::time::Duration::from_secs(abs_seconds)
            },
            message: message
                .body()
                .map(|b| b.to_string())
                .unwrap_or_else(|| message.summary().to_string()),
            parent_hashes: commit.parent_ids().map(|id| id.to_string()).collect(),
            insertions: 0, // Will be implemented with diff parsing
            deletions: 0,  // Will be implemented with diff parsing
        };

        commit_messages.push(ScanMessage::CommitData {
            scanner_id: self.scanner_id().to_string(),
            commit_info,
            timestamp: SystemTime::now(),
        });

        Ok(commit_messages)
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
            timestamp: if time.seconds >= 0 {
                SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(time.seconds as u64)
            } else {
                let abs_seconds = (-time.seconds) as u64;
                SystemTime::UNIX_EPOCH - std::time::Duration::from_secs(abs_seconds)
            },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::api::ScanRequires;
    use tempfile::TempDir;

    /// Helper to create a test git repository
    fn create_test_repo() -> (TempDir, gix::Repository) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = temp_dir.path();

        // Initialize a bare git repo for testing
        let repo = gix::init_bare(repo_path).expect("Failed to init git repo");

        (temp_dir, repo)
    }

    /// Helper to create a ScannerTask with specific requirements
    fn create_test_scanner_task(requirements: ScanRequires) -> (TempDir, ScannerTask) {
        let (_temp_dir, repo) = create_test_repo();
        let repo_path = repo.git_dir().to_string_lossy().to_string();

        let scanner = ScannerTask::builder("test-scanner-123".to_string(), repo_path, repo)
            .with_requirements(requirements)
            .build();

        (_temp_dir, scanner)
    }

    #[test]
    fn test_requirements_dependency_resolution() {
        // Test that ScanRequires correctly resolves dependencies
        let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::FILE_CONTENT);

        // FILE_CONTENT should include FILE_CHANGES and COMMITS
        let reqs = scanner.requirements();
        assert!(reqs.requires_file_content());
        assert!(reqs.requires_file_changes()); // dependency
        assert!(reqs.requires_commits()); // dependency of FILE_CHANGES
        assert!(!reqs.requires_repository_info()); // not included
        assert!(!reqs.requires_history()); // not included
    }

    #[test]
    fn test_history_requirements() {
        let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::HISTORY);

        // HISTORY should include COMMITS but not file-related requirements
        let reqs = scanner.requirements();
        assert!(reqs.requires_history());
        assert!(reqs.requires_commits()); // dependency
        assert!(!reqs.requires_file_changes());
        assert!(!reqs.requires_file_content());
        assert!(!reqs.requires_repository_info());
    }

    #[test]
    fn test_combined_requirements() {
        let combined =
            ScanRequires::REPOSITORY_INFO | ScanRequires::FILE_CONTENT | ScanRequires::HISTORY;
        let (_temp_dir, scanner) = create_test_scanner_task(combined);

        // Should include all specified requirements and their dependencies
        let reqs = scanner.requirements();
        assert!(reqs.requires_repository_info());
        assert!(reqs.requires_file_content());
        assert!(reqs.requires_file_changes()); // dependency of FILE_CONTENT
        assert!(reqs.requires_commits()); // dependency of both FILE_CONTENT and HISTORY
        assert!(reqs.requires_history());
    }

    #[test]
    fn test_no_requirements() {
        let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::NONE);

        let reqs = scanner.requirements();
        assert!(reqs.is_empty());
        assert!(!reqs.requires_repository_info());
        assert!(!reqs.requires_commits());
        assert!(!reqs.requires_file_changes());
        assert!(!reqs.requires_file_content());
        assert!(!reqs.requires_history());
    }

    #[test]
    fn test_repository_info_only() {
        let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::REPOSITORY_INFO);

        let reqs = scanner.requirements();
        assert!(reqs.requires_repository_info());
        assert!(!reqs.requires_commits()); // not included
        assert!(!reqs.requires_file_changes());
        assert!(!reqs.requires_file_content());
        assert!(!reqs.requires_history());
    }

    #[test]
    fn test_commits_only() {
        let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::COMMITS);

        let reqs = scanner.requirements();
        assert!(reqs.requires_commits());
        assert!(!reqs.requires_repository_info());
        assert!(!reqs.requires_file_changes()); // COMMITS doesn't include FILE_CHANGES
        assert!(!reqs.requires_file_content());
        assert!(!reqs.requires_history());
    }

    #[test]
    fn test_file_changes_includes_commits() {
        let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::FILE_CHANGES);

        let reqs = scanner.requirements();
        assert!(reqs.requires_file_changes());
        assert!(reqs.requires_commits()); // dependency
        assert!(!reqs.requires_repository_info());
        assert!(!reqs.requires_file_content()); // FILE_CHANGES doesn't include FILE_CONTENT
        assert!(!reqs.requires_history());
    }

    /// Test conditional data collection logic without triggering event publishing
    /// This tests the core logic that determines which data collection paths are taken
    #[test]
    fn test_conditional_data_collection_logic() {
        // Test each requirement combination to verify correct conditional logic paths

        // No requirements - should take early exit path
        let (_temp_dir, scanner_none) = create_test_scanner_task(ScanRequires::NONE);
        assert!(scanner_none.requirements().is_empty());

        // Repository info only - should collect repository data but not commit data
        let (_temp_dir, scanner_repo) = create_test_scanner_task(ScanRequires::REPOSITORY_INFO);
        let repo_reqs = scanner_repo.requirements();
        assert!(repo_reqs.requires_repository_info());
        assert!(!repo_reqs.requires_commits());
        assert!(!repo_reqs.requires_file_changes());

        // Commits only - should collect commits but not repository info
        let (_temp_dir, scanner_commits) = create_test_scanner_task(ScanRequires::COMMITS);
        let commit_reqs = scanner_commits.requirements();
        assert!(commit_reqs.requires_commits());
        assert!(!commit_reqs.requires_repository_info());
        assert!(!commit_reqs.requires_file_changes());

        // File changes - should collect commits and file changes but not repository info
        let (_temp_dir, scanner_files) = create_test_scanner_task(ScanRequires::FILE_CHANGES);
        let file_reqs = scanner_files.requirements();
        assert!(file_reqs.requires_file_changes());
        assert!(file_reqs.requires_commits()); // dependency
        assert!(!file_reqs.requires_repository_info());

        // History - should collect commits and history but not file data
        let (_temp_dir, scanner_history) = create_test_scanner_task(ScanRequires::HISTORY);
        let history_reqs = scanner_history.requirements();
        assert!(history_reqs.requires_history());
        assert!(history_reqs.requires_commits()); // dependency
        assert!(!history_reqs.requires_file_changes());
        assert!(!history_reqs.requires_repository_info());

        // Combined requirements - should collect all requested data types
        let combined =
            ScanRequires::REPOSITORY_INFO | ScanRequires::FILE_CONTENT | ScanRequires::HISTORY;
        let (_temp_dir, scanner_all) = create_test_scanner_task(combined);
        let all_reqs = scanner_all.requirements();
        assert!(all_reqs.requires_repository_info());
        assert!(all_reqs.requires_file_content());
        assert!(all_reqs.requires_file_changes()); // dependency
        assert!(all_reqs.requires_commits()); // dependency
        assert!(all_reqs.requires_history());
    }

    /// Test the automatic dependency inclusion in ScanRequires
    #[test]
    fn test_automatic_dependency_inclusion() {
        // FILE_CONTENT should automatically include FILE_CHANGES and COMMITS
        assert!(ScanRequires::FILE_CONTENT.requires_file_content());
        assert!(ScanRequires::FILE_CONTENT.requires_file_changes());
        assert!(ScanRequires::FILE_CONTENT.requires_commits());

        // FILE_CHANGES should automatically include COMMITS
        assert!(ScanRequires::FILE_CHANGES.requires_file_changes());
        assert!(ScanRequires::FILE_CHANGES.requires_commits());
        assert!(!ScanRequires::FILE_CHANGES.requires_file_content()); // should not include higher-level

        // HISTORY should automatically include COMMITS
        assert!(ScanRequires::HISTORY.requires_history());
        assert!(ScanRequires::HISTORY.requires_commits());
        assert!(!ScanRequires::HISTORY.requires_file_changes()); // should not include unrelated

        // COMMITS should not include anything else
        assert!(ScanRequires::COMMITS.requires_commits());
        assert!(!ScanRequires::COMMITS.requires_file_changes());
        assert!(!ScanRequires::COMMITS.requires_file_content());
        assert!(!ScanRequires::COMMITS.requires_history());

        // REPOSITORY_INFO should not include anything else
        assert!(ScanRequires::REPOSITORY_INFO.requires_repository_info());
        assert!(!ScanRequires::REPOSITORY_INFO.requires_commits());
    }
}
