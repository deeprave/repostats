//! Checkout Manager Implementation
//!
//! Manages temporary directory creation, template resolution, and cleanup
//! for historical file content checkout operations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
            Some(template) => {
                let resolved = template
                    .replace("{commit-id}", &vars.commit_id)
                    .replace("{sha256}", &vars.sha256)
                    .replace("{branch}", &vars.branch)
                    .replace("{repo}", &vars.repo);
                Ok(PathBuf::from(resolved))
            }
            None => {
                // Create temporary directory
                let temp_dir = std::env::temp_dir()
                    .join("repostats-checkout")
                    .join(&vars.commit_id);
                Ok(temp_dir)
            }
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
        TemplateVars {
            commit_id: "abc123".to_string(),
            sha256: "abc123def456".to_string(),
            branch: "main".to_string(),
            repo: "test-repo".to_string(),
        }
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

        // Should use temp directory
        assert!(resolved.to_string_lossy().contains("repostats-checkout"));
        assert!(resolved.to_string_lossy().contains("abc123"));
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
        let vars1 = TemplateVars {
            commit_id: "abc123".to_string(),
            ..create_test_vars()
        };
        let vars2 = TemplateVars {
            commit_id: "def456".to_string(),
            ..create_test_vars()
        };

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
}
