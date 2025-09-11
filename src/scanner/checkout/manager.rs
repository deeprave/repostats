//! Checkout Manager Implementation
//!
//! Manages temporary directory creation, template resolution, and cleanup
//! for historical file content checkout operations.

// Removed gix dependency - CheckoutManager is now directory-only
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;

// Removed Git operations import - CheckoutManager is now directory-only

/// Static regex for template variable matching to avoid recompilation
static TEMPLATE_VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{([a-zA-Z0-9\-_]+)\}").expect("Invalid regex pattern"));

/// Directory manager for handling checkout directory operations with automatic cleanup
///
/// Refactored to follow SRP - handles only directory operations, Git operations moved to ScannerTask
#[derive(Debug)]
pub struct CheckoutManager {
    /// Template for checkout directory paths
    checkout_template: String,
    /// Whether to keep files after processing
    pub keep_files: bool,
    /// Whether to force overwrite existing content
    force_overwrite: bool,
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
    /// Create a CheckoutManager with default settings
    pub async fn create() -> Self {
        Self::default()
    }

    /// Create a new directory-only checkout manager
    /// After refactoring: No longer requires repository path - focuses on directory management only
    pub fn new(checkout_template: String, force_overwrite: bool) -> Self {
        Self {
            checkout_template,
            keep_files: false,
            force_overwrite,
            active_checkouts: HashMap::new(),
        }
    }

    /// Prepare a checkout directory without Git operations (directory management only)
    /// This is the core method after SRP refactoring - handles only directory creation and tracking
    pub fn prepare_checkout_directory(&mut self, vars: &TemplateVars) -> CheckoutResult<PathBuf> {
        let checkout_path = self.resolve_template(vars)?;

        if checkout_path.exists() {
            if !self.force_overwrite {
                log::debug!(
                    "prepare_checkout_directory: Directory already exists, keeping existing: {}",
                    checkout_path.display()
                );
            } else {
                std::fs::remove_dir_all(&checkout_path).map_err(|e| CheckoutError::Io {
                    message: format!(
                        "Failed to remove existing checkout directory '{}': {}",
                        checkout_path.display(),
                        e
                    ),
                })?;
            }
        }

        std::fs::create_dir_all(&checkout_path).map_err(|e| CheckoutError::Io {
            message: format!(
                "Failed to create checkout directory '{}': {}",
                checkout_path.display(),
                e
            ),
        })?;

        let checkout_id = format!("{}-{}", vars.scanner_id, vars.commit_id);
        log::debug!(
            "prepare_checkout_directory: Created directory '{}' for checkout ID '{}'",
            checkout_path.display(),
            checkout_id
        );

        self.active_checkouts
            .insert(checkout_id, checkout_path.clone());

        Ok(checkout_path)
    }

    /// Resolve template variables to create directory path
    fn resolve_template(&self, vars: &TemplateVars) -> CheckoutResult<PathBuf> {
        let resolved_path =
            vars.render(&self.checkout_template)
                .map_err(|e| CheckoutError::Configuration {
                    message: format!(
                        "Failed to resolve checkout template '{}': {}",
                        self.checkout_template, e
                    ),
                })?;
        Ok(PathBuf::from(resolved_path))
    }

    /// Create a checkout manager with custom settings
    pub fn with_settings(
        checkout_template: String,
        keep_files: bool,
        force_overwrite: bool,
    ) -> Self {
        Self {
            checkout_template,
            keep_files,
            force_overwrite,
            active_checkouts: HashMap::new(),
        }
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
        // Use default template and settings
        Self::new(TemplateVars::DEFAULT_TEMPLATE.to_string(), false)
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

    #[test]
    fn test_checkout_manager_new() {
        let template = "/tmp/test-checkout-{commit-id}".to_string();
        let manager = CheckoutManager::new(template.clone(), false);

        assert_eq!(manager.checkout_template, template);
        assert!(!manager.keep_files);
        assert!(!manager.force_overwrite);
        assert_eq!(manager.active_checkout_count(), 0);
    }

    #[test]
    fn test_checkout_manager_with_settings() {
        let template = "/tmp/checkout-{repo}-{commit-id}".to_string();
        let manager = CheckoutManager::with_settings(
            template.clone(),
            true,  // keep_files
            false, // force_overwrite
        );

        assert_eq!(manager.checkout_template, template);
        assert!(manager.keep_files);
        assert!(!manager.force_overwrite);
        assert_eq!(manager.active_checkout_count(), 0);
    }

    #[test]
    fn test_template_resolution() {
        let template = "/tmp/{repo}-{commit-id}-{branch}-{sha256}".to_string();
        let manager = CheckoutManager::with_settings(template, false, false);
        let vars = create_test_vars();

        let resolved = manager.resolve_template(&vars).unwrap();
        let expected = PathBuf::from("/tmp/test-repo-abc123-main-abc123def456");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_template_resolution_with_default() {
        // Use the default template pattern
        let template = TemplateVars::DEFAULT_TEMPLATE.to_string();
        let manager = CheckoutManager::new(template, false);
        let vars = create_test_vars();

        let resolved = manager.resolve_template(&vars).unwrap();

        // Should resolve the default template pattern
        assert!(resolved.to_string_lossy().contains("checkout"));
        assert!(resolved.to_string_lossy().contains("test-repo"));
        assert!(resolved.to_string_lossy().contains("abc123"));
        assert!(resolved.to_string_lossy().contains("test-scanner"));
    }

    #[test]
    fn test_template_resolution_all_variables() {
        let template =
            "/tmp/{repo}-{commit-id}-{branch}-{sha256}-{tmpdir}-{pid}-{scanner-id}".to_string();
        let manager = CheckoutManager::with_settings(template, false, false);
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
        let template = "/tmp/{scanner-id}-{pid}".to_string();
        let manager = CheckoutManager::with_settings(template, false, false);
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
    fn test_prepare_checkout_directory_creates_directory() {
        // Use unique template to avoid conflicts with other tests
        let template = "/tmp/test-create-{scanner-id}-{commit-id}".to_string();
        let mut manager = CheckoutManager::new(template, false);
        let vars = create_test_vars();

        let checkout_path = manager.prepare_checkout_directory(&vars).unwrap();

        // Verify directory was created
        assert!(checkout_path.exists(), "Checkout directory should exist");
        assert!(checkout_path.is_dir(), "Should be a directory");

        // Verify path contains expected template variables
        let path_str = checkout_path.to_string_lossy();
        assert!(path_str.contains("abc123"), "Should contain commit-id");
        assert!(
            path_str.contains("test-scanner"),
            "Should contain scanner-id"
        );

        // Verify it's tracked as an active checkout
        let checkout_id = format!("{}-{}", vars.scanner_id, vars.commit_id);
        assert!(
            manager.is_checkout_active(&checkout_id),
            "Should track as active checkout"
        );

        // Cleanup
        manager.cleanup_all().unwrap();
    }

    #[test]
    fn test_prepare_checkout_directory_with_force_overwrite() {
        // Use unique template to avoid conflicts with other tests
        let template = "/tmp/force-test-{scanner-id}-{commit-id}".to_string();
        let mut manager = CheckoutManager::new(template, true); // force_overwrite = true
        let vars = create_test_vars();

        // Create directory first time
        let first_path = manager.prepare_checkout_directory(&vars).unwrap();
        assert!(first_path.exists(), "First directory should be created");

        // Write a test file to the directory (directory already exists from prepare_checkout_directory)
        let test_file = first_path.join("test.txt");
        std::fs::write(&test_file, "original content").unwrap();
        assert!(test_file.exists(), "Test file should exist");

        // Prepare directory again with force_overwrite - should succeed and clear existing content
        let second_path = manager.prepare_checkout_directory(&vars).unwrap();
        assert_eq!(first_path, second_path, "Should return same path");
        assert!(second_path.exists(), "Directory should still exist");
        assert!(
            !test_file.exists(),
            "Previous content should be removed with force overwrite"
        );

        // Cleanup
        manager.cleanup_all().unwrap();
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

    #[tokio::test]
    async fn test_prepare_checkout_directory_method() {
        // TDD test for directory-only CheckoutManager after refactoring

        // Use unique template path to avoid conflicts
        let template = "/tmp/method-test-{scanner-id}-{commit-id}".to_string();

        // Create CheckoutManager without repository dependency
        let mut manager = CheckoutManager::new(template, false);

        // Create template vars
        let vars = create_test_vars();

        let checkout_dir = manager.prepare_checkout_directory(&vars).unwrap();

        // Verify directory was created
        assert!(checkout_dir.exists(), "Directory should be created");
        assert!(checkout_dir.is_dir(), "Should be a directory");

        // Verify path contains expected components from resolved template
        let path_str = checkout_dir.to_string_lossy();
        assert!(
            path_str.contains("method-test"),
            "Should contain template pattern"
        );
        assert!(
            path_str.contains("test-scanner"),
            "Should contain scanner-id"
        );
        assert!(
            path_str.contains("abc123"),
            "Should contain commit_id (abc123)"
        );

        // Verify it's tracked as an active checkout
        let checkout_id = format!("{}-{}", vars.scanner_id, vars.commit_id);
        assert!(
            manager.is_checkout_active(&checkout_id),
            "Should track as active checkout"
        );

        // Cleanup
        manager.cleanup_all().unwrap();
    }
}
