//! Scanner Types and Enums
//!
//! Shared types and enums used throughout the scanner module.

use std::time::SystemTime;

/// Type of file change
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

/// File change information within a commit
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileChangeData {
    pub change_type: ChangeType,
    pub old_path: Option<String>,
    pub new_path: String,
    pub insertions: usize,
    pub deletions: usize,
    pub is_binary: bool,
}

/// Repository metadata information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepositoryData {
    pub path: String,
    pub url: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub default_branch: Option<String>,
    pub is_bare: bool,
    pub is_shallow: bool,
    pub work_dir: Option<String>,
    pub git_dir: String,
    // Query parameters (only included if they are not filtering/restrictive)
    pub git_ref: Option<String>,
    pub date_range: Option<String>, // Human-readable date range
    pub file_paths: Option<String>, // Comma-separated paths if not restrictive
    pub authors: Option<String>,    // Comma-separated authors if not restrictive
    pub max_commits: Option<usize>, // Only if not restrictive (None or very large)
}

impl RepositoryData {
    /// Create a new builder for RepositoryData
    pub fn builder() -> RepositoryDataBuilder {
        RepositoryDataBuilder::new()
    }
}

/// Builder for RepositoryData
#[derive(Debug, Default)]
pub struct RepositoryDataBuilder {
    pub path: Option<String>,
    pub url: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub default_branch: Option<String>,
    pub is_bare: Option<bool>,
    pub is_shallow: Option<bool>,
    pub work_dir: Option<String>,
    pub git_dir: Option<String>,
    pub git_ref: Option<String>,
    pub date_range: Option<String>,
    pub file_paths: Option<String>,
    pub authors: Option<String>,
    pub max_commits: Option<usize>,
}

impl RepositoryDataBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the repository path
    pub fn with_repository<S: Into<String>>(mut self, path: S) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Set repository information from gix::Repository
    pub fn with_repository_info(mut self, repo: &gix::Repository) -> Self {
        let config = repo.config_snapshot();

        // Extract repository metadata
        self.url = config.string("remote.origin.url").map(|s| s.to_string());
        self.git_dir = Some(repo.git_dir().to_string_lossy().to_string());
        self.work_dir = repo.workdir().map(|p| p.to_string_lossy().to_string());

        // Get repository name from URL or path
        if let Some(ref url_str) = self.url {
            // Extract repo name from URL (e.g., "repo.git" from "https://github.com/user/repo.git")
            self.name = url_str
                .split('/')
                .last()
                .map(|s| s.strip_suffix(".git").unwrap_or(s).to_string());
        } else if let Some(ref path) = self.path {
            // Extract from local path
            self.name = std::path::Path::new(path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string());
        }

        // Get description if available
        self.description = config
            .string("repository.description")
            .map(|s| s.to_string());

        // Get default branch by querying repository HEAD
        self.default_branch = config
            .string("init.defaultBranch")
            .or_else(|| config.string("branch.default"))
            .map(|s| s.to_string())
            .or_else(|| {
                // Try to get the current branch name from HEAD
                repo.head_name().ok().flatten().map(|head_name| {
                    // Extract branch name from full ref (e.g., "refs/heads/main" -> "main")
                    head_name.shorten().to_string()
                })
            })
            .or_else(|| {
                // Try to get remote default branch from origin/HEAD
                repo.find_reference("refs/remotes/origin/HEAD")
                    .ok()
                    .and_then(|origin_head| {
                        // Extract the branch name from the reference name
                        let name = origin_head.name();
                        Some(name.shorten().to_string())
                    })
            })
            .or_else(|| Some("main".to_string())); // final fallback only if all queries fail

        self.is_bare = Some(repo.is_bare());
        self.is_shallow = Some(repo.is_shallow());
        self
    }

    /// Set query parameters if they are not restrictive/filtering
    pub fn with_query(mut self, query_params: &crate::core::query::QueryParams) -> Self {
        // Only include git_ref if specified (None means no restriction)
        self.git_ref = query_params.git_ref.clone();

        // Only include date_range if it's very broad or unrestricted
        if let Some(ref date_range) = query_params.date_range {
            // Check if this is a restrictive date range
            // For now, include it as informational - could be refined later
            if let (Some(start), Some(end)) = (&date_range.start, &date_range.end) {
                self.date_range = Some(format!(
                    "{} to {}",
                    format_system_time(start),
                    format_system_time(end)
                ));
            } else if let Some(start) = &date_range.start {
                self.date_range = Some(format!("from {}", format_system_time(start)));
            } else if let Some(end) = &date_range.end {
                self.date_range = Some(format!("until {}", format_system_time(end)));
            }
        }

        // Include file_paths if they are specified
        if !query_params.file_paths.include.is_empty() {
            self.file_paths = Some(
                query_params
                    .file_paths
                    .include
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }

        // Include authors if they are specified
        if !query_params.authors.include.is_empty() {
            self.authors = Some(query_params.authors.include.join(", "));
        }

        // Include max_commits if specified
        self.max_commits = query_params.max_commits;

        self
    }

    /// Build the RepositoryData
    pub fn build(self) -> Result<RepositoryData, String> {
        Ok(RepositoryData {
            path: self.path.ok_or("Repository path is required")?,
            url: self.url,
            name: self.name,
            description: self.description,
            default_branch: self.default_branch,
            is_bare: self.is_bare.unwrap_or(false),
            is_shallow: self.is_shallow.unwrap_or(false),
            work_dir: self.work_dir,
            git_dir: self.git_dir.ok_or("Git directory is required")?,
            git_ref: self.git_ref,
            date_range: self.date_range,
            file_paths: self.file_paths,
            authors: self.authors,
            max_commits: self.max_commits,
        })
    }
}

/// Helper function to format SystemTime for human readability
fn format_system_time(time: &std::time::SystemTime) -> String {
    use chrono::{DateTime, Utc};

    match DateTime::<Utc>::try_from(*time) {
        Ok(datetime) => datetime.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        Err(_) => "invalid-time".to_string(),
    }
}

/// Scanner messages for repository scan data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ScanMessage {
    RepositoryData {
        scanner_id: String,
        timestamp: SystemTime,
        repository_data: RepositoryData,
    },
    CommitData {
        scanner_id: String,
        timestamp: SystemTime,
        commit_info: CommitInfo,
    },
    FileChange {
        scanner_id: String,
        timestamp: SystemTime,
        file_path: String,
        change_data: FileChangeData,
        commit_context: CommitInfo,
    },
    ScanCompleted {
        scanner_id: String,
        timestamp: SystemTime,
        stats: ScanStats,
    },
    ScanError {
        scanner_id: String,
        timestamp: SystemTime,
        error: String,
        context: String,
    },
}

impl ScanMessage {
    /// Get the message type string for queue publishing
    pub fn message_type(&self) -> &'static str {
        match self {
            ScanMessage::RepositoryData { .. } => "repository_data",
            ScanMessage::CommitData { .. } => "commit_data",
            ScanMessage::FileChange { .. } => "file_change",
            ScanMessage::ScanCompleted { .. } => "scan_completed",
            ScanMessage::ScanError { .. } => "scan_error",
        }
    }
}

/// Commit information structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: String,
    pub committer_name: String,
    pub committer_email: String,
    pub timestamp: SystemTime,
    pub message: String,
    pub parent_hashes: Vec<String>,
    pub insertions: usize,
    pub deletions: usize,
}

/// Scan statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScanStats {
    pub total_commits: usize,
    pub total_files_changed: usize,
    pub total_insertions: usize,
    pub total_deletions: usize,
    pub scan_duration: std::time::Duration,
}
