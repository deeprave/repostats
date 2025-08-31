//! Scanner Task Git Operations
//!
//! Git-related operations including repository access, commit scanning, and content reconstruction.

use crate::core::pattern_parser::AuthorPatternMatcher;
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
            repo.head_commit().map_err(|e| ScanError::Repository {
                message: format!("Failed to get HEAD commit: {}", e),
            })?
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

            commit_count += 1;
        }

        // Add scan completed message
        message_handler(ScanMessage::ScanCompleted {
            scanner_id: self.scanner_id().to_string(),
            timestamp: SystemTime::now(),
            stats: ScanStats {
                total_commits: commit_count,
                total_files_changed: 0, // TODO: Implement with file diff parsing in future issue
                total_insertions: 0,    // TODO: Implement with file diff parsing in future issue
                total_deletions: 0,     // TODO: Implement with file diff parsing in future issue
                scan_duration: std::time::Duration::from_millis(0), // TODO: Add timing in future optimization
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
            let repo = gix::open(&repository_path).map_err(|e| ScanError::Repository {
                message: format!("Failed to open repository: {}", e),
            })?;

            // Validate that the commit exists
            repo.rev_parse_single(commit_sha.as_str())
                .map_err(|e| ScanError::Repository {
                    message: format!("Failed to resolve commit '{}': {}", commit_sha, e),
                })?;

            // For Phase 6 initial implementation: read file from working directory
            // This demonstrates the API and basic functionality
            // Full historical reconstruction will be implemented in later phases
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::query::QueryParams;
    use crate::scanner::tests::helpers::{collect_scan_messages, count_commit_messages};
    use std::process::Command;
    use tempfile::TempDir;

    fn create_test_repository() -> (TempDir, gix::Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repository
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to init repository");

        // Configure git user
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to set user name");

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to set user email");

        // Create initial commit
        std::fs::write(repo_path.join("file1.txt"), "Initial content").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to add files");

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to create initial commit");

        // Create second commit with different author
        std::fs::write(repo_path.join("file2.txt"), "Second file").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to add second file");

        Command::new("git")
            .args([
                "-c",
                "user.name=Another User",
                "-c",
                "user.email=another@domain.org",
                "commit",
                "-m",
                "Second commit by another user",
            ])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to create second commit");

        // Create third commit
        std::fs::write(repo_path.join("file3.txt"), "Third file").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to add third file");

        Command::new("git")
            .args(["commit", "-m", "Third commit"])
            .current_dir(&repo_path)
            .output()
            .expect("Failed to create third commit");

        let repo = gix::open(repo_path).expect("Failed to open repository");
        (temp_dir, repo)
    }

    #[tokio::test]
    async fn test_commit_traversal_with_author_filtering() {
        let (_temp_dir, repo) = create_test_repository();

        // Create scanner task
        let scanner_task = ScannerTask::new_with_repository(
            "test-scanner".to_string(),
            repo.path().to_string_lossy().to_string(),
            repo,
        );

        // Test 1: Exact email match
        let query_params = QueryParams::new().with_authors(vec!["test@example.com".to_string()]);
        let commit_count = count_commit_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        assert_eq!(commit_count, 2, "Should find 2 commits by test@example.com");

        // Test 2: Email wildcard - domain pattern
        let query_params = QueryParams::new().with_authors(vec!["*@example.com".to_string()]);
        let commit_count = count_commit_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        assert_eq!(
            commit_count, 2,
            "Should find 2 commits with *@example.com pattern"
        );

        // Test 3: Email wildcard - broader domain pattern
        let query_params = QueryParams::new().with_authors(vec!["*@*.org".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(commit_count, 1, "Should find 1 commit with *@*.org pattern");

        // Test 4: Name wildcard pattern - case insensitive
        let query_params = QueryParams::new().with_authors(vec!["test*".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 2,
            "Should find 2 commits with 'test*' name pattern"
        );

        // Test 5: Name wildcard - partial word match (matches both "Test User" and "Another User")
        let query_params = QueryParams::new().with_authors(vec!["*User".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 3,
            "Should find 3 commits with '*User' name pattern (both Test User and Another User)"
        );

        // Test 6: Case insensitive name matching
        let query_params = QueryParams::new().with_authors(vec!["ANOTHER*".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 1,
            "Should find 1 commit with case-insensitive 'ANOTHER*' pattern"
        );
    }

    #[tokio::test]
    async fn test_complex_wildcard_patterns() {
        // Create a more complex test repository with varied email domains
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize repository
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .expect("git init");

        // Create commits with various authors
        let authors = [
            ("user1@aws.amazon.com", "David Nugent"),
            ("user2@amazon.com", "David L Nugent"),
            ("admin@google.com", "Admin User"),
            ("dev@aws.amazon.net", "david \"the maker\" nugent"),
        ];

        for (i, (email, name)) in authors.iter().enumerate() {
            std::fs::write(
                repo_path.join(format!("file{}.txt", i)),
                format!("content {}", i),
            )
            .unwrap();
            Command::new("git")
                .args(["add", "."])
                .current_dir(&repo_path)
                .output()
                .expect("git add");
            Command::new("git")
                .args([
                    "-c",
                    &format!("user.name={}", name),
                    "-c",
                    &format!("user.email={}", email),
                    "commit",
                    "-m",
                    &format!("Commit {}", i),
                ])
                .current_dir(&repo_path)
                .output()
                .expect("git commit");
        }

        let repo = gix::open(repo_path).expect("Failed to open repository");
        let scanner_task = ScannerTask::new_with_repository(
            "test-scanner".to_string(),
            repo.path().to_string_lossy().to_string(),
            repo,
        );

        // Test 1: Complex email domain pattern - should match aws.amazon.com and amazon.com
        let query_params = QueryParams::new().with_authors(vec!["*@*amazon.com".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 2,
            "Should match *@*amazon.com pattern (aws.amazon.com and amazon.com)"
        );

        // Test 2: Specific domain pattern
        let query_params = QueryParams::new().with_authors(vec!["*@amazon.com".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(commit_count, 1, "Should match exact *@amazon.com pattern");

        // Test 3: Case-insensitive name pattern with complex names
        let query_params = QueryParams::new().with_authors(vec!["david*".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 3,
            "Should match all David variants case-insensitively"
        );

        // Test 4: Pattern with special characters
        let query_params = QueryParams::new().with_authors(vec!["*\"the*\"*".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(commit_count, 1, "Should match name with special characters");
    }

    #[tokio::test]
    async fn test_email_auto_completion_integration() {
        // Test that auto-completion works through the full Git scanning pipeline
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize repository
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .expect("git init");

        // Create commits with specific email patterns for testing auto-completion
        let test_cases = [
            ("user@example.com", "John Smith"),
            ("admin@example.com", "Jane Admin"),
            ("developer@different.org", "Dev User"),
        ];

        for (i, (email, name)) in test_cases.iter().enumerate() {
            std::fs::write(
                repo_path.join(format!("file{}.txt", i)),
                format!("content {}", i),
            )
            .unwrap();
            Command::new("git")
                .args(["add", "."])
                .current_dir(&repo_path)
                .output()
                .expect("git add");
            Command::new("git")
                .args([
                    "-c",
                    &format!("user.name={}", name),
                    "-c",
                    &format!("user.email={}", email),
                    "commit",
                    "-m",
                    &format!("Commit {}", i),
                ])
                .current_dir(&repo_path)
                .output()
                .expect("git commit");
        }

        let repo = gix::open(repo_path).expect("Failed to open repository");
        let scanner_task = ScannerTask::new_with_repository(
            "test-scanner".to_string(),
            repo.path().to_string_lossy().to_string(),
            repo,
        );

        // Test 1: @example.com should auto-complete to *@example.com (match both users at example.com)
        let query_params = QueryParams::new().with_authors(vec!["@example.com".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 2,
            "Auto-completion '@example.com' → '*@example.com' should match 2 commits"
        );

        // Test 2: user@ should auto-complete to user@* (match user at any domain)
        let query_params = QueryParams::new().with_authors(vec!["user@".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 1,
            "Auto-completion 'user@' → 'user@*' should match 1 commit"
        );

        // Test 3: @ should auto-complete to *@* (match all email addresses)
        let query_params = QueryParams::new().with_authors(vec!["@".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 3,
            "Auto-completion '@' → '*@*' should match all 3 commits"
        );

        // Test 4: Explicit wildcards should still work (no auto-completion needed)
        let query_params = QueryParams::new().with_authors(vec!["*@*.org".to_string()]);
        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();
        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 1,
            "Explicit pattern '*@*.org' should match 1 commit (no auto-completion)"
        );
    }

    #[tokio::test]
    async fn test_commit_traversal_with_max_commits() {
        let (_temp_dir, repo) = create_test_repository();

        let scanner_task = ScannerTask::new_with_repository(
            "test-scanner".to_string(),
            repo.path().to_string_lossy().to_string(),
            repo,
        );

        // Limit to 2 commits
        let query_params = QueryParams::new().with_max_commits(Some(2));

        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();

        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(
            commit_count, 2,
            "Should return exactly 2 commits when max_commits is 2"
        );
    }

    #[tokio::test]
    async fn test_commit_traversal_with_git_ref() {
        let (_temp_dir, repo) = create_test_repository();

        // Create a branch at the second commit
        Command::new("git")
            .args(["branch", "test-branch", "HEAD~1"])
            .current_dir(repo.path())
            .output()
            .expect("Failed to create branch");

        let scanner_task = ScannerTask::new_with_repository(
            "test-scanner".to_string(),
            repo.path().to_string_lossy().to_string(),
            gix::open(repo.path()).unwrap(), // Re-open to get updated refs
        );

        // Start from test-branch (should have 2 commits)
        let query_params = QueryParams::new().with_git_ref(Some("test-branch".to_string()));

        let messages = collect_scan_messages(&scanner_task, Some(&query_params))
            .await
            .unwrap();

        let commit_count = messages
            .iter()
            .filter(|m| matches!(m, ScanMessage::CommitData { .. }))
            .count();
        assert_eq!(commit_count, 2, "test-branch should have 2 commits");
    }
}
