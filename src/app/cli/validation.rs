//! CLI argument validation utilities
//!
//! This module handles validation of CLI arguments for consistency and constraints.
//! It ensures repository paths exist, validates checkout functionality, and verifies
//! commit limits and plugin timeouts.

use crate::core::validation::ValidationError;
use crate::scanner::checkout::manager::TemplateVars;

use super::args::Args;

impl Args {
    /// Validate CLI arguments for consistency and constraints
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_repositories()?;
        self.validate_checkout_functionality()?;
        self.validate_commit_limits()?;
        self.validate_plugin_timeout()?;
        Ok(())
    }

    /// Validate repository arguments using normalized repository list
    fn validate_repositories(&self) -> Result<(), ValidationError> {
        // Use normalized repositories (empty list gets converted to current directory)
        let normalized_repos = self.normalized_repositories();

        // Validate that all repository entries are non-empty and valid
        for (index, repo) in normalized_repos.iter().enumerate() {
            // Ensure path is valid UTF-8 for proper validation
            let repo_str = match repo.to_str() {
                Some(s) => s,
                None => {
                    return Err(ValidationError::new(&format!(
                        "Repository path at index {} contains invalid UTF-8 characters. Please provide a valid UTF-8 path",
                        index
                    )));
                }
            };

            let trimmed = repo_str.trim();
            if trimmed.is_empty() {
                return Err(ValidationError::new(&format!(
                    "Repository at index {} cannot be empty",
                    index
                )));
            }

            // Check if path looks like a URL (don't validate filesystem for URLs)
            if trimmed.starts_with("http://")
                || trimmed.starts_with("https://")
                || trimmed.starts_with("git@")
            {
                // URL repositories are valid - skip filesystem checks
                continue;
            }

            // For local paths, validate existence and Git repository status
            if !repo.exists() {
                return Err(ValidationError::new(&format!(
                    "Repository path at index {} does not exist: '{}'",
                    index,
                    repo.display()
                )));
            }

            if !repo.is_dir() {
                return Err(ValidationError::new(&format!(
                    "Repository at index {} is not a directory: '{}'",
                    index,
                    repo.display()
                )));
            }

            // Check if it's a Git repository
            let git_dir = repo.join(".git");
            if !git_dir.exists() {
                return Err(ValidationError::new(&format!(
                    "Repository at index {} is not a Git repository (no .git directory found): '{}'",
                    index,
                    repo.display()
                )));
            }
        }

        Ok(())
    }

    /// Validate checkout functionality arguments
    fn validate_checkout_functionality(&self) -> Result<(), ValidationError> {
        let has_checkout_flags = self.checkout_dir.is_some()
            || self.checkout_keep
            || self.checkout_force
            || self.checkout_rev.is_some();

        if has_checkout_flags && self.repository.len() > 1 {
            log::debug!(
                "Checkout functionality error tracked in RS-29 (found {} repositories)",
                self.repository.len()
            );
            return Err(ValidationError::new(&format!(
                "Checkout functionality currently supports only a single repository (found {})",
                self.repository.len()
            )));
        }

        // Checkout flags require checkout-dir
        if (self.checkout_keep || self.checkout_force || self.checkout_rev.is_some())
            && self.checkout_dir.is_none()
        {
            return Err(ValidationError::new("Options --checkout-keep, --checkout-force, and --checkout-rev require --checkout-dir"));
        }

        // Validate checkout directory template if provided
        if let Some(checkout_template) = &self.checkout_dir {
            // Create test template variables for validation
            let test_vars = TemplateVars::new(
                "test-commit".to_string(),
                "test-sha".to_string(),
                "test-branch".to_string(),
                "test-repo".to_string(),
                "test-scanner".to_string(),
            );

            if let Err(e) = test_vars.render(checkout_template) {
                return Err(ValidationError::new(&format!(
                    "Invalid checkout directory template: {}",
                    e
                )));
            }
        }

        Ok(())
    }

    /// Validate commit limits and related arguments
    fn validate_commit_limits(&self) -> Result<(), ValidationError> {
        // Max commits validation
        if let Some(max) = self.max_commits {
            if max == 0 {
                return Err(ValidationError::new(
                    "Option --max-commits must be greater than 0",
                ));
            }
        }

        Ok(())
    }

    /// Validate plugin timeout argument
    fn validate_plugin_timeout(&self) -> Result<(), ValidationError> {
        if let Some(timeout_secs) = self.plugin_timeout {
            if timeout_secs < 5 {
                return Err(ValidationError::new(
                    "Option --plugin-timeout must be at least 5 seconds",
                ));
            }
        }
        Ok(())
    }
}
