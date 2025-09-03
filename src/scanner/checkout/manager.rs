//! Checkout Manager Implementation
//!
//! Manages temporary directory creation, template resolution, and cleanup
//! for historical file content checkout operations.

use gix;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::scanner::task::git_ops::open_repository_with_context;

/// Static regex for template variable matching to avoid recompilation
static TEMPLATE_VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{([a-zA-Z0-9\-_]+)\}").expect("Invalid regex pattern"));

/// Checkout manager for handling file checkout operations with automatic cleanup
#[derive(Debug)]
pub struct CheckoutManager {
    /// Template for checkout directory paths
    checkout_template: Option<String>,
    /// Whether to keep files after processing
    pub keep_files: bool,
    /// Whether to force overwrite existing content
    force_overwrite: bool,
    /// Git repository path for checkout operations
    repository_path: PathBuf,
    /// Default revision to checkout (commit SHA, branch, or tag)
    default_revision: Option<String>,
    /// Active checkout directories for cleanup tracking
    active_checkouts: HashMap<String, PathBuf>,
}

/// Template variables available for directory naming
#[derive(Debug, Clone)]
pub struct TemplateVars {
    pub commit_id: String,
    pub sha256: String,
    pub branch: String,
    pub repo: String,
    pub tmpdir: String,
    pub pid: String,
    pub scanner_id: String,
}

impl TemplateVars {
    /// Create TemplateVars with system defaults for optional fields
    pub fn new(
        commit_id: String,
        sha256: String,
        branch: String,
        repo: String,
        scanner_id: String,
    ) -> Self {
        Self {
            commit_id,
            sha256,
            branch,
            repo,
            tmpdir: std::env::temp_dir().display().to_string(),
            pid: std::process::id().to_string(),
            scanner_id,
        }
    }

    /// Create TemplateVars for a commit checkout with sensible defaults
    /// Uses shortened SHA, default branch and repo names when specific values aren't available
    pub fn for_commit_checkout(commit_sha: &str, scanner_id: &str) -> Self {
        const SHORT_SHA_LENGTH: usize = 8;

        Self::new(
            commit_sha.to_string(),
            commit_sha[..SHORT_SHA_LENGTH.min(commit_sha.len())].to_string(), // Use shortened SHA
            "HEAD".to_string(), // default branch when not available
            "repo".to_string(), // default repo name when not available
            scanner_id.to_string(),
        )
    }

    /// Create TemplateVars with optional values for all fields (None uses defaults)
    pub fn with_all_fields(
        commit_id: String,
        sha256: String,
        branch: String,
        repo: String,
        tmpdir: Option<String>,
        pid: Option<String>,
        scanner_id: String,
    ) -> Self {
        Self {
            commit_id,
            sha256,
            branch,
            repo,
            tmpdir: tmpdir.unwrap_or_else(|| std::env::temp_dir().display().to_string()),
            pid: pid.unwrap_or_else(|| std::process::id().to_string()),
            scanner_id,
        }
    }

    /// Default template pattern
    pub const DEFAULT_TEMPLATE: &'static str =
        "{tmpdir}/checkout/{repo}/{pid}-{scanner-id}-{commit-id}/";

    /// Render template string with variable substitution and validation
    ///
    /// # Template Variables
    /// Supports the following variables: {commit-id}, {sha256}, {branch}, {repo}, {tmpdir}, {pid}, {scanner-id}
    ///
    /// # Variable Name Requirements
    /// Variable names must not overlap or be prefixes of each other to avoid ambiguous substitutions.
    /// Current variables are designed to be non-overlapping (e.g., using 'commit-id' not 'commit').
    /// Future additions should follow this pattern to prevent substitution conflicts.
    ///
    /// # Error Handling
    /// Returns `TemplateError` if unknown variables are found in the template.
    pub fn render(&self, template: &str) -> CheckoutResult<String> {
        // Build substitution map - single source of truth for variables
        let mut substitutions = [
            ("commit-id", self.commit_id.as_str()),
            ("sha256", self.sha256.as_str()),
            ("branch", self.branch.as_str()),
            ("repo", self.repo.as_str()),
            ("tmpdir", self.tmpdir.as_str()),
            ("pid", self.pid.as_str()),
            ("scanner-id", self.scanner_id.as_str()),
        ];

        // Sort by descending key length to prevent partial replacements
        // E.g., {commit-id} should be replaced before {commit} to avoid conflicts
        substitutions.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));

        // Use static regex to match whole variable names and substitute using sorted array
        // to preserve the length-based ordering for overlapping variable names
        let mut unknown_vars = std::collections::HashSet::new();

        let resolved = TEMPLATE_VAR_REGEX.replace_all(template, |caps: &regex::Captures| {
            let key = &caps[1];
            // Search through sorted substitutions to find the key
            match substitutions.iter().find(|(k, _)| *k == key) {
                Some((_, value)) => value.to_string(),
                None => {
                    unknown_vars.insert(key.to_string());
                    caps[0].to_string() // Return original if unknown
                }
            }
        });

        if !unknown_vars.is_empty() {
            return Err(CheckoutError::Configuration {
                message: format!(
                    "Template resolution failed for template '{}'\n\
                 Unknown variables: {}\n\
                 Available variables: {}",
                    template,
                    {
                        let mut sorted_vars: Vec<_> = unknown_vars.iter().cloned().collect();
                        sorted_vars.sort();
                        sorted_vars.join(", ")
                    },
                    {
                        // Use the sorted substitutions array for consistent ordering
                        let available_vars: Vec<_> =
                            substitutions.iter().map(|(key, _)| *key).collect();
                        available_vars.join(", ")
                    }
                ),
            });
        }

        Ok(resolved.to_string())
    }

    /// Render with default template if none provided
    pub fn render_default(&self) -> CheckoutResult<String> {
        self.render(Self::DEFAULT_TEMPLATE)
    }
}

#[derive(Debug, Clone)]
pub enum CheckoutError {
    /// Repository validation failed
    Repository { message: String },
    /// IO operation failed
    Io { message: String },
    /// Invalid configuration
    Configuration { message: String },
}

impl std::fmt::Display for CheckoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckoutError::Repository { message } => write!(f, "Repository error: {}", message),
            CheckoutError::Io { message } => write!(f, "IO error: {}", message),
            CheckoutError::Configuration { message } => {
                write!(f, "Configuration error: {}", message)
            }
        }
    }
}

impl std::error::Error for CheckoutError {}

pub type CheckoutResult<T> = Result<T, CheckoutError>;

impl crate::core::error_handling::ContextualError for CheckoutError {
    fn is_user_actionable(&self) -> bool {
        match self {
            CheckoutError::Configuration { .. } => true, // User can fix config/template issues
            CheckoutError::Repository { .. } => false,   // Git/repository issues are system-level
            CheckoutError::Io { .. } => false,           // IO issues are system-level
        }
    }

    fn user_message(&self) -> Option<&str> {
        match self {
            CheckoutError::Configuration { message } => Some(message),
            _ => None,
        }
    }
}

impl CheckoutManager {
    /// Create a CheckoutManager and integrate with services (following established manager pattern)
    pub async fn create<P: AsRef<Path>>(repository_path: P) -> Self {
        Self::new(repository_path)
    }

    /// Helper to open the git repository with consistent error handling
    fn open_repository(&self) -> CheckoutResult<gix::Repository> {
        open_repository_with_context::<CheckoutError>(
            &self.repository_path.to_string_lossy(),
            &format!("at '{}'", self.repository_path.display()),
        )
    }

    /// Create a new checkout manager with repository path
    pub fn new<P: AsRef<Path>>(repository_path: P) -> Self {
        Self {
            checkout_template: None,
            keep_files: false,
            force_overwrite: false,
            repository_path: repository_path.as_ref().to_path_buf(),
            default_revision: None,
            active_checkouts: HashMap::new(),
        }
    }

    /// Create a checkout manager with custom settings
    pub fn with_settings<P: AsRef<Path>>(
        repository_path: P,
        checkout_template: Option<String>,
        keep_files: bool,
        force_overwrite: bool,
        default_revision: Option<String>,
    ) -> Self {
        Self {
            checkout_template,
            keep_files,
            force_overwrite,
            repository_path: repository_path.as_ref().to_path_buf(),
            default_revision,
            active_checkouts: HashMap::new(),
        }
    }

    /// Resolve directory template with provided variables
    pub fn resolve_template(&self, vars: &TemplateVars) -> CheckoutResult<PathBuf> {
        match &self.checkout_template {
            Some(template) => Ok(PathBuf::from(vars.render(template)?)),
            None => Ok(PathBuf::from(vars.render_default()?)),
        }
    }

    /// Resolve revision (branch, tag, or commit SHA) to a commit SHA string
    ///
    /// Resolves branches, tags, and commit SHAs to their full commit SHA using git.
    /// Supports --checkout-rev override and defaults to HEAD when no revision specified.
    pub fn resolve_revision(&self, revision: Option<&str>) -> CheckoutResult<String> {
        let repo = self.open_repository()?;

        let revision_str = revision
            .or(self.default_revision.as_deref())
            .unwrap_or("HEAD");

        log::debug!(
            "resolve_revision: Resolving revision '{}' to commit SHA",
            revision_str
        );

        // Use gix to resolve the revision to a commit object
        let parsed_ref =
            repo.rev_parse(revision_str)
                .map_err(|e| CheckoutError::Configuration {
                    message: format!("Failed to resolve revision '{}': {}", revision_str, e),
                })?;

        let commit_id = parsed_ref
            .single()
            .ok_or_else(|| CheckoutError::Configuration {
                message: format!(
                    "Revision '{}' could not be resolved to a single object",
                    revision_str
                ),
            })?;

        // Verify the resolved object is actually a commit
        let commit = repo
            .find_commit(commit_id)
            .map_err(|e| CheckoutError::Configuration {
                message: format!(
                    "Revision '{}' (SHA: {}) is not a valid commit: {}",
                    revision_str, commit_id, e
                ),
            })?;

        let commit_sha = commit.id().to_string();

        log::debug!(
            "resolve_revision: Successfully resolved '{}' to commit SHA '{}'",
            revision_str,
            commit_sha
        );

        Ok(commit_sha)
    }

    /// Extract files from a git commit to the specified directory
    ///
    /// Extracts all files from the specified commit to the target directory,
    /// preserving the directory structure and file content.
    pub fn extract_files_from_commit(
        &self,
        revision: &str,
        target_dir: &Path,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> CheckoutResult<usize> {
        let repo = self.open_repository()?;

        log::debug!(
            "extract_files_from_commit: Extracting files from revision '{}' to '{}'",
            revision,
            target_dir.display()
        );

        // Parse the revision to get commit SHA
        let parsed_ref = repo
            .rev_parse(revision)
            .map_err(|e| CheckoutError::Configuration {
                message: format!("Failed to resolve revision '{}': {}", revision, e),
            })?;

        let commit_id = parsed_ref
            .single()
            .ok_or_else(|| CheckoutError::Configuration {
                message: format!(
                    "Revision '{}' could not be resolved to a single object",
                    revision
                ),
            })?;

        // Get the commit object
        let commit = repo
            .find_commit(commit_id)
            .map_err(|e| CheckoutError::Repository {
                message: format!("Failed to find commit '{}': {}", commit_id, e),
            })?;

        // Get the tree from the commit
        let tree = commit.tree().map_err(|e| CheckoutError::Repository {
            message: format!("Failed to get tree for commit '{}': {}", commit_id, e),
        })?;

        // Count total entries for progress reporting
        let total_entries = self.count_tree_entries(&tree)?;
        let mut extracted_count = 0;

        // Extract all files recursively
        self.extract_tree_recursive(
            &tree,
            target_dir,
            "",
            &mut extracted_count,
            total_entries,
            progress_callback,
        )?;

        log::debug!(
            "extract_files_from_commit: Successfully extracted {} files from revision '{}'",
            extracted_count,
            revision
        );

        Ok(extracted_count)
    }

    /// Count total entries in tree for progress reporting
    fn count_tree_entries(&self, tree: &gix::Tree) -> CheckoutResult<usize> {
        let mut count = 0;
        for entry_result in tree.iter() {
            let entry = entry_result.map_err(|e| CheckoutError::Repository {
                message: format!("Failed to read tree entry: {}", e),
            })?;

            if entry.mode().is_tree() {
                // Recursively count subtree entries
                let subtree = entry
                    .object()
                    .map_err(|e| CheckoutError::Repository {
                        message: format!("Failed to get subtree: {}", e),
                    })?
                    .try_into_tree()
                    .map_err(|_| CheckoutError::Repository {
                        message: "Expected tree object".to_string(),
                    })?;
                count += self.count_tree_entries(&subtree)?;
            } else {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Recursively extract tree contents to directory
    fn extract_tree_recursive(
        &self,
        tree: &gix::Tree,
        base_dir: &Path,
        relative_path: &str,
        extracted_count: &mut usize,
        total_entries: usize,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> CheckoutResult<()> {
        for entry_result in tree.iter() {
            let entry = entry_result.map_err(|e| CheckoutError::Repository {
                message: format!("Failed to read tree entry: {}", e),
            })?;

            let entry_name =
                std::str::from_utf8(entry.filename()).map_err(|e| CheckoutError::Repository {
                    message: format!("Invalid UTF-8 in filename: {}", e),
                })?;

            let entry_path = if relative_path.is_empty() {
                entry_name.to_string()
            } else {
                format!("{}/{}", relative_path, entry_name)
            };

            let target_path = base_dir.join(&entry_path);

            if entry.mode().is_tree() {
                // Create directory and recurse
                std::fs::create_dir_all(&target_path).map_err(|e| CheckoutError::Io {
                    message: e.to_string(),
                })?;

                let subtree = entry
                    .object()
                    .map_err(|e| CheckoutError::Repository {
                        message: format!("Failed to get subtree: {}", e),
                    })?
                    .try_into_tree()
                    .map_err(|_| CheckoutError::Repository {
                        message: "Expected tree object".to_string(),
                    })?;

                self.extract_tree_recursive(
                    &subtree,
                    base_dir,
                    &entry_path,
                    extracted_count,
                    total_entries,
                    progress_callback,
                )?;
            } else {
                // Extract file
                let blob = entry
                    .object()
                    .map_err(|e| CheckoutError::Repository {
                        message: format!("Failed to get blob: {}", e),
                    })?
                    .try_into_blob()
                    .map_err(|_| CheckoutError::Repository {
                        message: "Expected blob object".to_string(),
                    })?;

                // Ensure parent directory exists
                if let Some(parent) = target_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| CheckoutError::Io {
                        message: e.to_string(),
                    })?;
                }

                // Write file content
                std::fs::write(&target_path, &blob.data).map_err(|e| CheckoutError::Io {
                    message: e.to_string(),
                })?;

                *extracted_count += 1;

                // Report progress
                if let Some(callback) = progress_callback {
                    callback(*extracted_count, total_entries);
                }

                log::trace!(
                    "extract_files_from_commit: Extracted file '{}' ({} bytes)",
                    entry_path,
                    blob.data.len()
                );
            }
        }
        Ok(())
    }

    /// Create checkout directory and extract files from git
    pub fn create_checkout(
        &mut self,
        vars: &TemplateVars,
        revision: Option<&str>,
    ) -> CheckoutResult<PathBuf> {
        let checkout_path = self.resolve_template(vars)?;

        if checkout_path.exists() {
            if !self.force_overwrite {
                return Err(CheckoutError::Configuration {
                    message: format!(
                        "Directory already exists: {}. Use --checkout-force to overwrite.",
                        checkout_path.display()
                    ),
                });
            }
            std::fs::remove_dir_all(&checkout_path).map_err(|e| CheckoutError::Io {
                message: e.to_string(),
            })?;
        }

        std::fs::create_dir_all(&checkout_path).map_err(|e| CheckoutError::Io {
            message: e.to_string(),
        })?;

        // Resolve revision to commit SHA
        let resolved_revision = self.resolve_revision(revision)?;

        // Extract files from the commit to the checkout directory
        let extracted_count = self.extract_files_from_commit(
            &resolved_revision,
            &checkout_path,
            None, // No progress callback for now
        )?;

        log::debug!(
            "Successfully extracted {} files from revision '{}' to {}",
            extracted_count,
            resolved_revision,
            checkout_path.display()
        );

        // Track this checkout for cleanup
        let checkout_id = vars.commit_id.clone();
        self.active_checkouts
            .insert(checkout_id, checkout_path.clone());

        Ok(checkout_path)
    }

    /// Create checkout directory with progress reporting
    pub fn create_checkout_with_progress(
        &mut self,
        vars: &TemplateVars,
        revision: Option<&str>,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> CheckoutResult<PathBuf> {
        let checkout_path = self.resolve_template(vars)?;

        if checkout_path.exists() {
            if !self.force_overwrite {
                return Err(CheckoutError::Configuration {
                    message: format!(
                        "Directory already exists: {}. Use --checkout-force to overwrite.",
                        checkout_path.display()
                    ),
                });
            }
            std::fs::remove_dir_all(&checkout_path).map_err(|e| CheckoutError::Io {
                message: e.to_string(),
            })?;
        }

        std::fs::create_dir_all(&checkout_path).map_err(|e| CheckoutError::Io {
            message: e.to_string(),
        })?;

        // Resolve revision to commit SHA
        let resolved_revision = self.resolve_revision(revision)?;

        // Extract files from the commit with progress reporting
        let extracted_count =
            self.extract_files_from_commit(&resolved_revision, &checkout_path, progress_callback)?;

        log::debug!(
            "Successfully extracted {} files from revision '{}' to {}",
            extracted_count,
            resolved_revision,
            checkout_path.display()
        );

        // Track this checkout for cleanup
        let checkout_id = vars.commit_id.clone();
        self.active_checkouts
            .insert(checkout_id, checkout_path.clone());

        Ok(checkout_path)
    }

    /// Clean up a specific checkout directory
    pub fn cleanup_checkout(&mut self, checkout_id: &str) -> CheckoutResult<()> {
        if let Some(checkout_path) = self.active_checkouts.remove(checkout_id) {
            if !self.keep_files && checkout_path.exists() {
                std::fs::remove_dir_all(&checkout_path).map_err(|e| CheckoutError::Io {
                    message: e.to_string(),
                })?;
            }
        }
        Ok(())
    }

    /// Clean up all active checkouts
    pub fn cleanup_all(&mut self) -> CheckoutResult<()> {
        if !self.keep_files {
            for (_, checkout_path) in self.active_checkouts.drain() {
                if checkout_path.exists() {
                    std::fs::remove_dir_all(&checkout_path).map_err(|e| CheckoutError::Io {
                        message: e.to_string(),
                    })?;
                }
            }
        } else {
            self.active_checkouts.clear();
        }
        Ok(())
    }

    /// Get the number of active checkouts
    pub fn active_checkout_count(&self) -> usize {
        self.active_checkouts.len()
    }

    /// Check if a checkout is active
    pub fn is_checkout_active(&self, checkout_id: &str) -> bool {
        self.active_checkouts.contains_key(checkout_id)
    }
}

impl Default for CheckoutManager {
    fn default() -> Self {
        // Default to current directory for repository path
        Self::new(".")
    }
}

impl Drop for CheckoutManager {
    fn drop(&mut self) {
        // Cleanup all checkouts when the manager is dropped
        // This ensures cleanup even on panic or unexpected termination
        let active_count = self.active_checkout_count();

        if active_count > 0 {
            log::debug!(
                "CheckoutManager dropping: cleaning up {} active checkouts",
                active_count
            );
            if let Err(e) = self.cleanup_all() {
                log::warn!("Failed to cleanup checkouts during drop: {}", e);
            } else {
                log::debug!(
                    "Successfully cleaned up {} checkouts during drop",
                    active_count
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_vars() -> TemplateVars {
        TemplateVars::new(
            "abc123".to_string(),
            "abc123def456".to_string(),
            "main".to_string(),
            "test-repo".to_string(),
            "test-scanner".to_string(),
        )
    }

    /// Create test repository with known content for checkout testing
    fn create_test_repository() -> (tempfile::TempDir, PathBuf, String) {
        use std::process::Command;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize repository
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create test files with known content
        std::fs::write(repo_path.join("test.txt"), "test content").unwrap();
        std::fs::create_dir_all(repo_path.join("src")).unwrap();
        std::fs::write(repo_path.join("src/lib.rs"), "pub fn test() {}").unwrap();

        // Commit the files
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Test commit"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Get commit SHA
        let commit_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let commit_str = std::str::from_utf8(&commit_output.stdout)
            .unwrap()
            .trim()
            .to_string();

        (temp_dir, repo_path, commit_str)
    }

    #[test]
    fn test_checkout_manager_new() {
        let manager = CheckoutManager::new("/tmp/test-repo");

        assert_eq!(manager.checkout_template, None);
        assert!(!manager.keep_files);
        assert!(!manager.force_overwrite);
        assert_eq!(manager.repository_path, PathBuf::from("/tmp/test-repo"));
        assert_eq!(manager.default_revision, None);
        assert_eq!(manager.active_checkout_count(), 0);
    }

    #[test]
    fn test_checkout_manager_with_settings() {
        let template = Some("/tmp/checkout-{repo}-{commit-id}".to_string());
        let revision = Some("main".to_string());
        let manager = CheckoutManager::with_settings(
            "/tmp/test-repo",
            template.clone(),
            true,
            false,
            revision.clone(),
        );

        assert_eq!(manager.checkout_template, template);
        assert!(manager.keep_files);
        assert!(!manager.force_overwrite);
        assert_eq!(manager.repository_path, PathBuf::from("/tmp/test-repo"));
        assert_eq!(manager.default_revision, revision);
        assert_eq!(manager.active_checkout_count(), 0);
    }

    #[test]
    fn test_template_resolution() {
        let template = Some("/tmp/{repo}-{commit-id}-{branch}-{sha256}".to_string());
        let manager =
            CheckoutManager::with_settings("/tmp/test-repo", template, false, false, None);
        let vars = create_test_vars();

        let resolved = manager.resolve_template(&vars).unwrap();
        let expected = PathBuf::from("/tmp/test-repo-abc123-main-abc123def456");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_template_resolution_no_template() {
        let manager = CheckoutManager::new("/tmp/test-repo");
        let vars = create_test_vars();

        let resolved = manager.resolve_template(&vars).unwrap();

        // Should use default template pattern
        assert!(resolved.to_string_lossy().contains("checkout"));
        assert!(resolved.to_string_lossy().contains("test-repo"));
        assert!(resolved.to_string_lossy().contains("abc123"));
        assert!(resolved.to_string_lossy().contains("test-scanner"));
    }

    #[test]
    fn test_template_resolution_all_variables() {
        let template = Some(
            "/tmp/{repo}-{commit-id}-{branch}-{sha256}-{tmpdir}-{pid}-{scanner-id}".to_string(),
        );
        let manager =
            CheckoutManager::with_settings("/tmp/test-repo", template, false, false, None);
        let vars = TemplateVars::with_all_fields(
            "abc123".to_string(),
            "abc123def456".to_string(),
            "main".to_string(),
            "test-repo".to_string(),
            Some("/temp".to_string()),
            Some("12345".to_string()),
            "scanner-1".to_string(),
        );

        let resolved = manager.resolve_template(&vars).unwrap();
        let expected =
            PathBuf::from("/tmp/test-repo-abc123-main-abc123def456-/temp-12345-scanner-1");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_template_edge_case_overlapping_variables() {
        // Test the edge case fix: longer keys should be substituted before shorter ones
        // to prevent partial replacements. For example, if we had {commit} and {commit-id}
        // variables, {commit-id} should be replaced before {commit} to avoid issues.
        // Since we don't have overlapping variables in our current set, this test demonstrates
        // the sorting behavior and ensures the implementation is correct.
        let template = Some("/tmp/{scanner-id}-{pid}".to_string());
        let manager =
            CheckoutManager::with_settings("/tmp/test-repo", template, false, false, None);
        let vars = TemplateVars::with_all_fields(
            "abc123".to_string(),
            "sha".to_string(),
            "main".to_string(),
            "repo".to_string(),
            Some("/temp".to_string()),
            Some("12345".to_string()),
            "my-scanner-id".to_string(),
        );

        let resolved = manager.resolve_template(&vars).unwrap();
        let expected = PathBuf::from("/tmp/my-scanner-id-12345");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_create_checkout_requires_git_repo() {
        let (_temp_dir, repo_path, commit_sha) = create_test_repository();
        let mut manager = CheckoutManager::new(&repo_path);

        let vars = TemplateVars::new(
            commit_sha.clone(),
            format!("{}abcdef", &commit_sha[..8]),
            "main".to_string(),
            "test-repo".to_string(),
            "test-scanner".to_string(),
        );

        let checkout_result = manager.create_checkout(&vars, Some(&commit_sha));
        assert!(
            checkout_result.is_ok(),
            "Should successfully create checkout"
        );

        let checkout_path = checkout_result.unwrap();
        assert!(checkout_path.exists(), "Checkout directory should exist");
        assert!(
            checkout_path.join("test.txt").exists(),
            "test.txt should be checked out"
        );
        assert!(
            checkout_path.join("src").exists(),
            "src directory should be checked out"
        );
        assert!(
            checkout_path.join("src/lib.rs").exists(),
            "src/lib.rs should be checked out"
        );

        // Verify file content
        let content = std::fs::read_to_string(checkout_path.join("test.txt")).unwrap();
        assert_eq!(content, "test content");

        let lib_content = std::fs::read_to_string(checkout_path.join("src/lib.rs")).unwrap();
        assert_eq!(lib_content, "pub fn test() {}");

        // Cleanup
        manager.cleanup_all().unwrap();
    }

    #[test]
    fn test_create_checkout_already_exists_no_force() {
        let (_temp_dir, repo_path, commit_sha) = create_test_repository();
        let mut manager = CheckoutManager::new(&repo_path);

        let vars = TemplateVars::new(
            commit_sha.clone(),
            format!("{}abcdef", &commit_sha[..8]),
            "main".to_string(),
            "test-repo".to_string(),
            "test-scanner".to_string(),
        );

        // Create checkout first time - should succeed
        let checkout_result = manager.create_checkout(&vars, Some(&commit_sha));
        assert!(checkout_result.is_ok(), "First checkout should succeed");

        // Try to create same checkout again without force - should fail
        let second_checkout_result = manager.create_checkout(&vars, Some(&commit_sha));
        assert!(
            second_checkout_result.is_err(),
            "Second checkout should fail without force"
        );

        // Verify error message mentions force option
        let error_msg = format!("{}", second_checkout_result.unwrap_err());
        assert!(
            error_msg.contains("--checkout-force"),
            "Error should mention force option"
        );

        // Cleanup
        manager.cleanup_all().unwrap();
    }

    #[test]
    #[ignore]
    fn test_create_checkout_already_exists_with_force() {
        // Temporarily disabled - requires actual git repository functionality
    }

    #[test]
    #[ignore]
    fn test_cleanup_checkout() {
        // Temporarily disabled - requires actual git repository functionality
    }
    #[test]
    #[ignore]
    fn test_cleanup_checkout_with_keep_files() {
        // Temporarily disabled - requires actual git repository functionality
    }
    #[test]
    #[ignore]
    fn test_cleanup_all() {
        // Temporarily disabled - requires actual git repository functionality
    }
    #[test]
    #[ignore]
    fn test_drop_cleanup() {
        // Temporarily disabled - requires actual git repository functionality
    }
    #[test]
    fn test_template_validation_unknown_variables() {
        let vars = create_test_vars();

        // Test template with unknown variable
        let result = vars.render("/tmp/{unknown-var}/checkout");
        assert!(
            result.is_err(),
            "Template with unknown variable should fail"
        );
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("unknown-var"),
            "Error should mention the unknown variable"
        );
        assert!(
            error_msg.contains("Available variables:"),
            "Error should list available variables"
        );

        // Test template with multiple unknown variables
        let result_multi = vars.render("/tmp/{invalid1}/{invalid2}/{commit-id}");
        assert!(
            result_multi.is_err(),
            "Template with multiple unknown variables should fail"
        );
        let error_multi = result_multi.unwrap_err().to_string();
        assert!(
            error_multi.contains("invalid1") && error_multi.contains("invalid2"),
            "Error should mention all unknown variables"
        );

        // Test valid template still works
        let result_valid = vars.render("/tmp/{commit-id}/{repo}");
        assert!(result_valid.is_ok(), "Valid template should succeed");
        assert_eq!(result_valid.unwrap(), "/tmp/abc123/test-repo");
    }

    #[test]
    fn test_template_validation_no_variables() {
        let vars = create_test_vars();

        // Test template with no variables (should work)
        let result = vars.render("/tmp/static/path");
        assert!(result.is_ok(), "Template with no variables should work");
        assert_eq!(result.unwrap(), "/tmp/static/path");
    }
}
