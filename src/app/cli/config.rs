//! TOML configuration file parsing and loading
//!
//! This module handles loading and parsing of TOML configuration files,
//! including default config file discovery and validation of config values.

use crate::core::validation::ValidationError;
use std::path::PathBuf;

use super::args::Args;

/// Field type for determining appropriate parsing method in TOML configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// Fields containing file paths (require path validation)
    PathField,
    /// Fields containing regular strings (no path validation)
    StringField,
}

impl Args {
    /// Load config file and return both updated Args and raw TOML config
    pub async fn parse_config_file_with_raw_config(
        args: &mut Self,
        config_file: Option<PathBuf>,
    ) -> Option<toml::Table> {
        let config_path = match config_file {
            Some(path) => {
                // User specified a config file - it must exist
                if !path.exists() {
                    eprintln!(
                        "Error: The specified configuration file does not exist: {}",
                        path.display()
                    );
                    std::process::exit(1);
                }
                Some(path)
            }
            None => {
                // Use default config path if it exists
                let default_path =
                    dirs::config_dir().map(|d| d.join("Repostats").join("repostats.toml"));
                match default_path {
                    Some(path) if path.exists() => Some(path),
                    _ => None, // No config file to load
                }
            }
        };

        // If we have a config path, load and parse it
        if let Some(path) = config_path {
            match tokio::fs::read_to_string(&path).await {
                Ok(contents) => match toml::from_str::<toml::Table>(&contents) {
                    Ok(config) => {
                        if let Err(e) = Self::apply_toml_values(args, &config) {
                            eprintln!(
                                "Error in configuration file validation {}: {}",
                                path.display(),
                                e
                            );
                            std::process::exit(1);
                        }
                        Some(config) // Return the raw config
                    }
                    Err(e) => {
                        eprintln!("Error parsing configuration file {}: {}", path.display(), e);
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("Error reading configuration file {}: {}", path.display(), e);
                    std::process::exit(1);
                }
            }
        } else {
            None // No config file found
        }
    }

    /// Apply string array field from TOML config (handles both single string and array formats)
    fn apply_string_array_field(
        config: &toml::Table,
        key: &str,
        target: &mut Vec<String>,
    ) -> Result<(), ValidationError> {
        if let Some(value) = config.get(key) {
            let mut temp_strings = Vec::new();

            if let Some(str_val) = value.as_str() {
                temp_strings.push(str_val.to_string());
            } else if let Some(array_val) = value.as_array() {
                for item in array_val {
                    if let Some(item_str) = item.as_str() {
                        temp_strings.push(item_str.to_string());
                    }
                }
            }

            let deduplicated = Self::parse_comma_separated_strings(&temp_strings);
            target.extend(deduplicated);
        }
        Ok(())
    }

    /// Apply TOML configuration values to Args
    pub fn apply_toml_values(args: &mut Self, config: &toml::Table) -> Result<(), ValidationError> {
        if let Some(repo_value) = config.get("repository") {
            let mut repo_paths = Vec::new();

            if let Some(repo_str) = repo_value.as_str() {
                // Single repository format: repository = "path"
                repo_paths.push(PathBuf::from(repo_str));
            } else if let Some(repo_array) = repo_value.as_array() {
                // Array format: repository = ["path1", "path2"]
                for item in repo_array {
                    if let Some(path_str) = item.as_str() {
                        repo_paths.push(PathBuf::from(path_str));
                    }
                }
            }

            // Apply deduplication and add to existing repositories
            // Config values are added first, CLI args take precedence through later processing
            let deduplicated = Self::parse_comma_separated_paths(&repo_paths);
            args.repository.extend(deduplicated);
        }
        if let Some(plugin_dir) = config.get("plugin-dir").and_then(|v| v.as_str()) {
            args.plugin_dir = Some(plugin_dir.to_string());
        }

        // Handle plugin exclusions (support both single string and array formats)
        if let Some(exclusions_value) = config.get("exclude-plugin") {
            let mut exclusion_strings = Vec::new();

            if let Some(exclusion_str) = exclusions_value.as_str() {
                exclusion_strings.push(exclusion_str.to_string());
            } else if let Some(exclusion_array) = exclusions_value.as_array() {
                for item in exclusion_array {
                    if let Some(exclusion_str) = item.as_str() {
                        exclusion_strings.push(exclusion_str.to_string());
                    }
                }
            }

            let deduplicated = Self::parse_comma_separated_strings(&exclusion_strings);
            args.plugin_exclusions.extend(deduplicated);
        }
        if let Some(color) = config.get("color").and_then(|v| v.as_bool()) {
            args.color = Some(color);
        }
        if let Some(no_color_enabled) = config.get("no-color").and_then(|v| v.as_bool()) {
            // Legacy support: Convert no-color=true to color=Some(false), no-color=false to color=Some(true)
            // This maintains backward compatibility while providing clear semantics
            let color_enabled = !no_color_enabled;
            args.color = Some(color_enabled);
        }
        if let Some(log_level) = config.get("log-level").and_then(|v| v.as_str()) {
            args.log_level = Some(log_level.to_string());
        }
        if let Some(log_file) = config.get("log-file").and_then(|v| v.as_str()) {
            if log_file.eq_ignore_ascii_case("none") || log_file == "-" {
                args.log_file = None; // Magic values "none" and "-" disable file logging
            } else {
                args.log_file = Some(PathBuf::from(log_file));
            }
        }
        if let Some(log_format) = config.get("log-format").and_then(|v| v.as_str()) {
            args.log_format = Some(log_format.to_string());
        }
        if let Some(since) = config.get("since").and_then(|v| v.as_str()) {
            args.since = Some(since.to_string());
        }
        if let Some(until) = config.get("until").and_then(|v| v.as_str()) {
            args.until = Some(until.to_string());
        }

        // Handle author fields (support both single string and array formats)
        Self::apply_string_array_field(config, "author", &mut args.author)?;
        Self::apply_string_array_field(config, "exclude-author", &mut args.exclude_author)?;

        // Handle file filtering fields
        Self::apply_array_field(config, "files", &mut args.files)?;
        Self::apply_array_field(config, "exclude-files", &mut args.exclude_files)?;
        Self::apply_array_field(config, "paths", &mut args.paths)?;
        Self::apply_array_field(config, "exclude-paths", &mut args.exclude_paths)?;
        Self::apply_array_field(config, "extensions", &mut args.extensions)?;
        Self::apply_array_field(config, "exclude-extensions", &mut args.exclude_extensions)?;

        // Handle git reference
        if let Some(git_ref) = config.get("ref").and_then(|v| v.as_str()) {
            args.git_ref = Some(git_ref.to_string());
        }

        // Handle commit limits
        if let Some(max_commits) = config.get("max-commits").and_then(|v| v.as_integer()) {
            args.max_commits = Some(max_commits as usize);
        }
        if let Some(max_files) = config
            .get("max-files-per-commit")
            .and_then(|v| v.as_integer())
        {
            args.max_files_per_commit = Some(max_files as usize);
        }
        // Handle mutually exclusive merge commit flags from TOML
        if let Some(no_merge) = config.get("no-merge-commits").and_then(|v| v.as_bool()) {
            args.no_merge_commits = no_merge;
        } else if let Some(merge) = config.get("merge-commits").and_then(|v| v.as_bool()) {
            args.no_merge_commits = !merge;
        }

        Ok(())
    }

    /// Get the field type for a TOML configuration key
    pub fn get_field_type(key: &str) -> FieldType {
        match key {
            // Path-based fields that require path validation
            "files" | "exclude-files" | "paths" | "exclude-paths" => FieldType::PathField,
            // String-based fields that don't require path validation
            "author" | "exclude-author" | "extensions" | "exclude-extensions" => {
                FieldType::StringField
            }
            // Default to string field for unknown keys (safer default)
            _ => FieldType::StringField,
        }
    }

    /// Helper method to apply array field from TOML config with deduplication
    fn apply_array_field(
        config: &toml::Table,
        key: &str,
        target: &mut Vec<String>,
    ) -> Result<(), ValidationError> {
        if let Some(value) = config.get(key) {
            let mut temp_strings = Vec::new();

            if let Some(str_val) = value.as_str() {
                // Single string format
                temp_strings.push(str_val.to_string());
            } else if let Some(array_val) = value.as_array() {
                // Array format
                for item in array_val {
                    if let Some(item_str) = item.as_str() {
                        temp_strings.push(item_str.to_string());
                    }
                }
            }

            // Apply appropriate parsing based on explicit field type mapping
            let deduplicated = match Self::get_field_type(key) {
                FieldType::PathField => Self::parse_comma_separated_path_patterns(&temp_strings)?,
                FieldType::StringField => Self::parse_comma_separated_strings(&temp_strings),
            };

            target.extend(deduplicated);
        }
        Ok(())
    }
}
