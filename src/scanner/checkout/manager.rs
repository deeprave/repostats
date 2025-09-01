//! Checkout Manager Implementation
//!
//! Manages temporary directory creation, template resolution, and cleanup
//! for historical file content checkout operations.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;

/// Static regex for template variable matching to avoid recompilation
static TEMPLATE_VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{([a-zA-Z0-9\-_]+)\}").expect("Invalid regex pattern"));

/// Checkout manager for handling file checkout operations with automatic cleanup
#[derive(Debug)]
pub struct CheckoutManager {
    /// Template for checkout directory paths
    checkout_template: Option<String>,
    /// Whether to keep files after processing
    keep_files: bool,
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
        let substitutions = [
            ("commit-id", self.commit_id.as_str()),
            ("sha256", self.sha256.as_str()),
            ("branch", self.branch.as_str()),
            ("repo", self.repo.as_str()),
            ("tmpdir", self.tmpdir.as_str()),
            ("pid", self.pid.as_str()),
            ("scanner-id", self.scanner_id.as_str()),
        ]
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>();

        // Use static regex to match whole variable names in curly braces and substitute
        let mut unknown_vars = std::collections::HashSet::new();

        let resolved = TEMPLATE_VAR_REGEX.replace_all(template, |caps: &regex::Captures| {
            let key = &caps[1];
            match substitutions.get(key) {
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
                    let mut sorted_available: Vec<_> = substitutions.keys().cloned().collect();
                    sorted_available.sort();
                    sorted_available.join(", ")
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

    #[error("Template resolution error: {0}")]
    TemplateError(String),

    #[error("Directory already exists and force_overwrite is false: {0}")]
    DirectoryExists(PathBuf),
}

pub type CheckoutResult<T> = Result<T, CheckoutError>;

impl CheckoutManager {
    /// Create a new checkout manager
    pub fn new() -> Self {
        Self {
            checkout_template: None,
            keep_files: false,
            force_overwrite: false,
            active_checkouts: HashMap::new(),
        }
    }

    /// Create a checkout manager with custom settings
    pub fn with_settings(
        checkout_template: Option<String>,
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

    /// Resolve directory template with provided variables
    pub fn resolve_template(&self, vars: &TemplateVars) -> CheckoutResult<PathBuf> {
        match &self.checkout_template {
            Some(template) => Ok(PathBuf::from(vars.render(template)?)),
            None => Ok(PathBuf::from(vars.render_default()?)),
        }
    }

    /// Create checkout directory and return the path
    pub fn create_checkout_dir(&mut self, vars: &TemplateVars) -> CheckoutResult<PathBuf> {
        let checkout_path = self.resolve_template(vars)?;

        if checkout_path.exists() {
            if !self.force_overwrite {
                return Err(CheckoutError::DirectoryExists(checkout_path));
            }
            std::fs::remove_dir_all(&checkout_path)?;
        }

        std::fs::create_dir_all(&checkout_path)?;

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
        Self::new()
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
    use tempfile::TempDir;

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
        let manager = CheckoutManager::new();

        assert_eq!(manager.checkout_template, None);
        assert!(!manager.keep_files);
        assert!(!manager.force_overwrite);
        assert_eq!(manager.active_checkout_count(), 0);
    }

    #[test]
    fn test_checkout_manager_with_settings() {
        let template = Some("/tmp/checkout-{repo}-{commit-id}".to_string());
        let manager = CheckoutManager::with_settings(template.clone(), true, false);

        assert_eq!(manager.checkout_template, template);
        assert!(manager.keep_files);
        assert!(!manager.force_overwrite);
        assert_eq!(manager.active_checkout_count(), 0);
    }

    #[test]
    fn test_template_resolution() {
        let template = Some("/tmp/{repo}-{commit-id}-{branch}-{sha256}".to_string());
        let manager = CheckoutManager::with_settings(template, false, false);
        let vars = create_test_vars();

        let resolved = manager.resolve_template(&vars).unwrap();
        let expected = PathBuf::from("/tmp/test-repo-abc123-main-abc123def456");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_template_resolution_no_template() {
        let manager = CheckoutManager::new();
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
        let template = Some("/tmp/{scanner-id}-{pid}".to_string());
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
    fn test_create_checkout_dir() {
        let temp_dir = TempDir::new().unwrap();
        let template = Some(format!(
            "{}/checkout-{{commit-id}}",
            temp_dir.path().display()
        ));
        let mut manager = CheckoutManager::with_settings(template, false, false);
        let vars = create_test_vars();

        let checkout_path = manager.create_checkout_dir(&vars).unwrap();

        // Directory should exist
        assert!(checkout_path.exists());
        assert!(checkout_path.is_dir());

        // Should be tracked
        assert!(manager.is_checkout_active("abc123"));
        assert_eq!(manager.active_checkout_count(), 1);
    }

    #[test]
    fn test_create_checkout_dir_already_exists_no_force() {
        let temp_dir = TempDir::new().unwrap();
        let template = Some(format!(
            "{}/checkout-{{commit-id}}",
            temp_dir.path().display()
        ));
        let mut manager = CheckoutManager::with_settings(template, false, false);
        let vars = create_test_vars();

        // Create the directory first
        let checkout_path = manager.resolve_template(&vars).unwrap();
        std::fs::create_dir_all(&checkout_path).unwrap();

        // Should fail without force
        let result = manager.create_checkout_dir(&vars);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CheckoutError::DirectoryExists(_)
        ));
    }

    #[test]
    fn test_create_checkout_dir_already_exists_with_force() {
        let temp_dir = TempDir::new().unwrap();
        let template = Some(format!(
            "{}/checkout-{{commit-id}}",
            temp_dir.path().display()
        ));
        let mut manager = CheckoutManager::with_settings(template, false, true);
        let vars = create_test_vars();

        // Create the directory first with a test file
        let checkout_path = manager.resolve_template(&vars).unwrap();
        std::fs::create_dir_all(&checkout_path).unwrap();
        std::fs::write(checkout_path.join("test.txt"), "test content").unwrap();

        // Should succeed with force
        let result_path = manager.create_checkout_dir(&vars).unwrap();

        assert!(result_path.exists());
        assert!(!result_path.join("test.txt").exists()); // Old content removed
        assert!(manager.is_checkout_active("abc123"));
    }

    #[test]
    fn test_cleanup_checkout() {
        let temp_dir = TempDir::new().unwrap();
        let template = Some(format!(
            "{}/checkout-{{commit-id}}",
            temp_dir.path().display()
        ));
        let mut manager = CheckoutManager::with_settings(template, false, false);
        let vars = create_test_vars();

        let checkout_path = manager.create_checkout_dir(&vars).unwrap();
        assert!(checkout_path.exists());
        assert!(manager.is_checkout_active("abc123"));

        // Cleanup should remove directory and tracking
        manager.cleanup_checkout("abc123").unwrap();

        assert!(!checkout_path.exists());
        assert!(!manager.is_checkout_active("abc123"));
        assert_eq!(manager.active_checkout_count(), 0);
    }

    #[test]
    fn test_cleanup_checkout_with_keep_files() {
        let temp_dir = TempDir::new().unwrap();
        let template = Some(format!(
            "{}/checkout-{{commit-id}}",
            temp_dir.path().display()
        ));
        let mut manager = CheckoutManager::with_settings(template, true, false);
        let vars = create_test_vars();

        let checkout_path = manager.create_checkout_dir(&vars).unwrap();
        assert!(checkout_path.exists());

        // Cleanup should not remove directory but should remove tracking
        manager.cleanup_checkout("abc123").unwrap();

        assert!(checkout_path.exists()); // Files kept
        assert!(!manager.is_checkout_active("abc123")); // Tracking removed
    }

    #[test]
    fn test_cleanup_all() {
        let temp_dir = TempDir::new().unwrap();
        let template = Some(format!(
            "{}/checkout-{{commit-id}}",
            temp_dir.path().display()
        ));
        let mut manager = CheckoutManager::with_settings(template, false, false);

        // Create multiple checkouts
        let vars1 = TemplateVars::new(
            "abc123".to_string(),
            "abc123def789".to_string(),
            "main".to_string(),
            "test-repo".to_string(),
            "test-scanner".to_string(),
        );
        let vars2 = TemplateVars::new(
            "def456".to_string(),
            "def456abc789".to_string(),
            "main".to_string(),
            "test-repo".to_string(),
            "test-scanner".to_string(),
        );

        let path1 = manager.create_checkout_dir(&vars1).unwrap();
        let path2 = manager.create_checkout_dir(&vars2).unwrap();

        assert_eq!(manager.active_checkout_count(), 2);
        assert!(path1.exists());
        assert!(path2.exists());

        // Cleanup all should remove all directories
        manager.cleanup_all().unwrap();

        assert_eq!(manager.active_checkout_count(), 0);
        assert!(!path1.exists());
        assert!(!path2.exists());
    }

    #[test]
    fn test_drop_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let template = Some(format!(
            "{}/checkout-{{commit-id}}",
            temp_dir.path().display()
        ));
        let vars = create_test_vars();

        let checkout_path = {
            let mut manager = CheckoutManager::with_settings(template, false, false);
            let path = manager.create_checkout_dir(&vars).unwrap();
            assert!(path.exists());
            path
        }; // manager drops here

        // Directory should be cleaned up by Drop
        assert!(!checkout_path.exists());
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
