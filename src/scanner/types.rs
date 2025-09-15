//! Scanner Types and Enums
//!
//! Shared types and enums used throughout the scanner module.

use std::path::PathBuf;
use std::time::SystemTime;

/// Bitflags for scanner requirements from plugins
///
/// These flags indicate what data the scanner needs to provide based on
/// plugin requirements. Flags automatically include their dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanRequires(u64);

impl ScanRequires {
    /// No requirements
    pub const NONE: Self = Self(0);

    /// Repository information (metadata)
    pub const REPOSITORY_INFO: Self = Self(1 << 0);

    /// Commit information
    pub const COMMITS: Self = Self(1 << 1);

    /// File change information (includes commits)
    pub const FILE_CHANGES: Self = Self((1 << 2) | Self::COMMITS.0);

    /// File content at HEAD/tag/commit (includes file changes)
    pub const FILE_CONTENT: Self = Self((1 << 3) | Self::FILE_CHANGES.0);

    /// Full history traversal (includes commits)
    pub const HISTORY: Self = Self((1 << 4) | Self::COMMITS.0);

    /// Suppress progress indicators (e.g., spinner) when plugins produce their own stdout output
    pub const SUPPRESS_PROGRESS: Self = Self(1 << 5);

    /// File metadata / change info (explicit request; includes file changes and commits)
    pub const FILE_INFO: Self = Self((1 << 6) | Self::FILE_CHANGES.0);

    /// Create from raw bits
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Get raw bits
    pub const fn bits(&self) -> u64 {
        self.0
    }

    /// Check if no requirements are set
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Check if a specific requirement is set
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine requirements with bitwise OR
    pub const fn union(&self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Get requirements that are in both sets
    pub const fn intersection(&self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Remove requirements
    pub const fn difference(&self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Check if repository info is required
    pub const fn requires_repository_info(&self) -> bool {
        self.contains(Self::REPOSITORY_INFO)
    }

    /// Check if commits are required
    pub const fn requires_commits(&self) -> bool {
        self.contains(Self::COMMITS)
    }

    /// Check if file changes are required
    pub const fn requires_file_changes(&self) -> bool {
        self.contains(Self::FILE_CHANGES)
    }

    /// Check if file content is required
    pub const fn requires_file_content(&self) -> bool {
        self.contains(Self::FILE_CONTENT)
    }

    /// Check if history traversal is required
    pub const fn requires_history(&self) -> bool {
        self.contains(Self::HISTORY)
    }

    /// Check if progress indicators should be suppressed
    pub const fn suppresses_progress(&self) -> bool {
        self.contains(Self::SUPPRESS_PROGRESS)
    }

    /// Check if file info explicitly requested
    pub const fn requires_file_info(&self) -> bool {
        self.contains(Self::FILE_INFO)
    }
}

impl Default for ScanRequires {
    fn default() -> Self {
        Self::NONE
    }
}

impl std::ops::BitOr for ScanRequires {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for ScanRequires {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

impl std::ops::BitAnd for ScanRequires {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.intersection(rhs)
    }
}

impl std::ops::BitAndAssign for ScanRequires {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = self.intersection(rhs);
    }
}

impl std::fmt::Display for ScanRequires {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut requirements = Vec::new();

        // Show all top-level requirements that were explicitly requested
        // Dependencies are automatically included but not separately displayed

        if self.requires_repository_info() {
            requirements.push("RepositoryInfo");
        }

        // Show the highest level in the file content hierarchy
        if self.requires_file_content() {
            requirements.push("FileContent");
        } else if self.requires_file_info() {
            requirements.push("FileInfo");
        } else if self.requires_file_changes() {
            requirements.push("FileChanges");
        }

        // History is independent of file content hierarchy
        if self.requires_history() {
            requirements.push("History");
        }

        // Only show commits if no higher-level requirement includes it
        if self.requires_commits()
            && !self.requires_file_changes()
            && !self.requires_file_content()
            && !self.requires_history()
        {
            requirements.push("Commits");
        }

        if self.suppresses_progress() {
            requirements.push("SuppressProgress");
        }

        if requirements.is_empty() {
            write!(f, "None")
        } else {
            write!(f, "{}", requirements.join(" | "))
        }
    }
}

/// Type of file change
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// Full path to the specific checked-out file (if FILE_CONTENT requirement is active)
    pub checkout_path: Option<PathBuf>,
    /// File's last modified time (epoch seconds) at the scanned commit (approx: commit time unless refined later)
    pub file_modified_epoch: Option<u64>,
    /// File mode (permission/type bits) as recorded in the git tree (e.g. "BlobExecutable", "Blob", "Link", etc.)
    pub file_mode: Option<String>,
}

/// Repository metadata information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepositoryData {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
    pub is_bare: bool,
    pub is_shallow: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_dir: Option<String>,
    pub git_dir: String,
    // Query parameters (only included if they are not filtering/restrictive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_range: Option<String>, // Human-readable date range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_paths: Option<String>, // Comma-separated paths if not restrictive
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<String>, // Comma-separated authors if not restrictive
    #[serde(skip_serializing_if = "Option::is_none")]
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
        // Preserve original path case by using the path from with_repository(),
        // falling back to repo.workdir() only if no path was set
        self.work_dir = self
            .path
            .clone()
            .or_else(|| repo.workdir().map(|p| p.to_string_lossy().to_string()));

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
    ScanStarted {
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
            ScanMessage::ScanStarted { .. } => "scan_started",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_requires_basic_operations() {
        let none = ScanRequires::NONE;
        let repo_info = ScanRequires::REPOSITORY_INFO;
        let commits = ScanRequires::COMMITS;

        assert!(none.is_empty());
        assert!(!repo_info.is_empty());
        assert!(repo_info.requires_repository_info());
        assert!(!repo_info.requires_commits());
        assert!(commits.requires_commits());
    }

    #[test]
    fn test_scan_requires_automatic_dependencies() {
        // FILE_CHANGES should include COMMITS
        let file_changes = ScanRequires::FILE_CHANGES;
        assert!(file_changes.requires_file_changes());
        assert!(file_changes.requires_commits()); // Should automatically include commits

        // FILE_CONTENT should include FILE_CHANGES and COMMITS
        let file_content = ScanRequires::FILE_CONTENT;
        assert!(file_content.requires_file_content());
        assert!(file_content.requires_file_changes()); // Should automatically include file changes
        assert!(file_content.requires_commits()); // Should automatically include commits

        // HISTORY should include COMMITS
        let history = ScanRequires::HISTORY;
        assert!(history.requires_history());
        assert!(history.requires_commits()); // Should automatically include commits
    }

    #[test]
    fn test_scan_requires_bitwise_operations() {
        let repo_info = ScanRequires::REPOSITORY_INFO;
        let commits = ScanRequires::COMMITS;

        let combined = repo_info | commits;
        assert!(combined.requires_repository_info());
        assert!(combined.requires_commits());

        let intersection = combined & repo_info;
        assert!(intersection.requires_repository_info());
        assert!(!intersection.requires_commits());
    }

    #[test]
    fn test_scan_requires_display() {
        let none = ScanRequires::NONE;
        assert_eq!(format!("{}", none), "None");

        let repo_info = ScanRequires::REPOSITORY_INFO;
        assert_eq!(format!("{}", repo_info), "RepositoryInfo");

        let combined = ScanRequires::REPOSITORY_INFO | ScanRequires::COMMITS;
        assert_eq!(format!("{}", combined), "RepositoryInfo | Commits");

        let file_content = ScanRequires::FILE_CONTENT;
        // FILE_CONTENT includes FILE_CHANGES which includes COMMITS
        // Display should show FileContent (the highest level requirement)
        assert_eq!(format!("{}", file_content), "FileContent");

        // Test that both HISTORY and FILE_CHANGES are shown when both are explicitly requested
        let history_and_file_changes = ScanRequires::HISTORY | ScanRequires::FILE_CHANGES;
        assert_eq!(
            format!("{}", history_and_file_changes),
            "FileChanges | History"
        );
    }

    #[test]
    fn test_scan_requires_complex_combinations() {
        let everything =
            ScanRequires::REPOSITORY_INFO | ScanRequires::FILE_CONTENT | ScanRequires::HISTORY;

        assert!(everything.requires_repository_info());
        assert!(everything.requires_file_content());
        assert!(everything.requires_file_changes()); // included by FILE_CONTENT
        assert!(everything.requires_commits()); // included by both FILE_CONTENT and HISTORY
        assert!(everything.requires_history());
    }

    #[test]
    fn test_scan_requires_default() {
        let default = ScanRequires::default();
        assert!(default.is_empty());
        assert_eq!(default, ScanRequires::NONE);
    }
}
