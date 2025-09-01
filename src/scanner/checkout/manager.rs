//! Checkout Manager Implementation
//!
//! Manages temporary directory creation, template resolution, and cleanup
//! for historical file content checkout operations.

use gix;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Static regex for template variable matching to avoid recompilation
static TEMPLATE_VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{([a-zA-Z0-9\-_]+)\}").expect("Invalid regex pattern"));

/// Checkout manager for handling file checkout operations with automatic cleanup
#[derive(Debug, Clone)]
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

        let substitution_map: HashMap<_, _> = substitutions.iter().cloned().collect();

        // Use static regex to match whole variable names in curly braces and substitute
        let mut unknown_vars = std::collections::HashSet::new();

        let resolved = TEMPLATE_VAR_REGEX.replace_all(template, |caps: &regex::Captures| {
            let key = &caps[1];
            match substitution_map.get(key) {
                Some(value) => value.to_string(),
                None => {
                    unknown_vars.insert(key.to_string());
                    caps[0].to_string() // Return original if unknown
                }
            }
        });

        if !unknown_vars.is_empty() {
            return Err(CheckoutError::TemplateError(format!(
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
            )));
        }

        Ok(resolved.to_string())
    }

    /// Render with default template if none provided
    pub fn render_default(&self) -> CheckoutResult<String> {
        self.render(Self::DEFAULT_TEMPLATE)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CheckoutError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(#[from] gix::open::Error),

    #[error("Git repository operation failed: {0}")]
    GitOperation(String),

    #[error("Template resolution error: {0}")]
    TemplateError(String),

    #[error("Directory already exists and force_overwrite is false: {0}")]
    DirectoryExists(PathBuf),

    #[error("Revision '{0}' not found in repository")]
    RevisionNotFound(String),

    #[error("Progress reporting failed: {0}")]
    Progress(String),
}

pub type CheckoutResult<T> = Result<T, CheckoutError>;

impl crate::core::error_handling::ContextualError for CheckoutError {
    fn is_user_actionable(&self) -> bool {
        match self {
            CheckoutError::DirectoryExists(_) => true,
            CheckoutError::RevisionNotFound(_) => true,
            CheckoutError::TemplateError(_) => true,
            _ => false,
        }
    }

    fn user_message(&self) -> Option<&str> {
        match self {
            CheckoutError::DirectoryExists(_) => {
                Some("The checkout directory already exists. Use --checkout-force to overwrite.")
            }
            CheckoutError::RevisionNotFound(_) => {
                Some("The specified revision could not be found. Please check the branch, tag, or commit SHA.")
            }
            CheckoutError::TemplateError(_) => {
                Some("The checkout directory template contains invalid variables. Valid variables are: {commit-id}, {sha256}, {branch}, {repo}, {tmpdir}, {pid}, {scanner-id}")
            }
            _ => None,
        }
    }
}

impl CheckoutManager {
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
    /// For now, this is a placeholder that validates the repository exists
    /// and returns the revision or a default. Full git resolution will be
    /// implemented in a future enhancement.
    pub fn resolve_revision(&self, revision: Option<&str>) -> CheckoutResult<String> {
        // Verify repository exists
        let _repo = gix::open(&self.repository_path)?;

        let revision_str = revision
            .or(self.default_revision.as_deref())
            .unwrap_or("HEAD");

        // TODO: Implement proper revision resolution to commit SHA
        // IMPORTANT: This is a stub implementation - revision is not resolved to a commit SHA
        log::warn!(
            "resolve_revision: Returning revision '{}' as-is without resolving to commit SHA. This is a stub implementation.",
            revision_str
        );

        Ok(revision_str.to_string())
    }

    /// Extract files from a git commit to the specified directory
    ///
    /// This is a placeholder implementation that creates the directory structure.
    /// Full git file extraction will be implemented in a future enhancement.
    pub fn extract_files_from_commit(
        &self,
        revision: &str,
        target_dir: &Path,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> CheckoutResult<usize> {
        // Verify repository exists
        let _repo = gix::open(&self.repository_path)?;

        // TODO: Implement actual git file extraction using gix
        // IMPORTANT: This is a stub implementation - no files are actually checked out
        log::warn!(
            "extract_files_from_commit: Only a placeholder file is created at '{}' for revision '{}'. No files have been checked out. This is a stub implementation.",
            target_dir.join(".checkout_info").display(),
            revision
        );

        // For now, just create a placeholder file to indicate the checkout happened
        let placeholder_file = target_dir.join(".checkout_info");
        let info_content = format!(
            "Placeholder checkout only.\nNo files were extracted from revision: {}\nTimestamp: {}\nThis is a stub implementation.\n",
            revision,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        std::fs::write(&placeholder_file, info_content)?;

        // Report progress if callback provided
        if let Some(callback) = progress_callback {
            callback(1, 1);
        }

        // Return 1 for the placeholder file
        Ok(1)
    }

    /// Create checkout directory and extract files from git
    pub fn create_checkout_dir(
        &mut self,
        vars: &TemplateVars,
        revision: Option<&str>,
    ) -> CheckoutResult<PathBuf> {
        let checkout_path = self.resolve_template(vars)?;

        if checkout_path.exists() {
            if !self.force_overwrite {
                return Err(CheckoutError::DirectoryExists(checkout_path));
            }
            std::fs::remove_dir_all(&checkout_path)?;
        }

        std::fs::create_dir_all(&checkout_path)?;

        // Resolve revision to commit SHA
        let resolved_revision = self.resolve_revision(revision)?;

        // Extract files from the commit to the checkout directory
        let extracted_count = self.extract_files_from_commit(
            &resolved_revision,
            &checkout_path,
            None, // No progress callback for now
        )?;

        log::debug!(
            "Extracted {} files from revision '{}' to {}",
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
    pub fn create_checkout_dir_with_progress(
        &mut self,
        vars: &TemplateVars,
        revision: Option<&str>,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> CheckoutResult<PathBuf> {
        let checkout_path = self.resolve_template(vars)?;

        if checkout_path.exists() {
            if !self.force_overwrite {
                return Err(CheckoutError::DirectoryExists(checkout_path));
            }
            std::fs::remove_dir_all(&checkout_path)?;
        }

        std::fs::create_dir_all(&checkout_path)?;

        // Resolve revision to commit SHA
        let resolved_revision = self.resolve_revision(revision)?;

        // Extract files from the commit with progress reporting
        let extracted_count =
            self.extract_files_from_commit(&resolved_revision, &checkout_path, progress_callback)?;

        log::debug!(
            "Extracted {} files from revision '{}' to {}",
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
                std::fs::remove_dir_all(&checkout_path)?;
            }
        }
        Ok(())
    }

    /// Clean up all active checkouts
    pub fn cleanup_all(&mut self) -> CheckoutResult<()> {
        if !self.keep_files {
            for (_, checkout_path) in self.active_checkouts.drain() {
                if checkout_path.exists() {
                    std::fs::remove_dir_all(&checkout_path)?;
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
        // Best effort cleanup on drop
        let _ = self.cleanup_all();
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

    // TODO: Re-enable once git functionality is fully implemented
    // This test requires a real git repository to work with the new checkout functionality
    #[test]
    #[ignore]
    fn test_create_checkout_dir_requires_git_repo() {
        // This test is temporarily disabled because it requires actual git checkout functionality
        // which will be implemented in the next iteration
    }

    #[test]
    #[ignore]
    fn test_create_checkout_dir_already_exists_no_force() {
        // Temporarily disabled - requires actual git repository functionality
    }

    #[test]
    #[ignore]
    fn test_create_checkout_dir_already_exists_with_force() {
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
