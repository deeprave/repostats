//! Plugin Error Handling
//!
//! Comprehensive error types for plugin operations including loading, execution,
//! compatibility checking, and runtime failures.

use std::fmt;

/// Result type alias for plugin operations
pub type PluginResult<T> = std::result::Result<T, PluginError>;

/// Comprehensive error types for plugin system operations
#[derive(Debug, Clone, PartialEq)]
pub enum PluginError {
    /// Plugin not found in registry
    PluginNotFound { plugin_name: String },

    /// Plugin API version incompatible with system
    VersionIncompatible { message: String },

    /// Plugin failed to load or initialize
    LoadError { plugin_name: String, cause: String },

    /// Plugin execution failed
    ExecutionError {
        plugin_name: String,
        operation: String,
        cause: String,
    },

    /// Async operation error
    AsyncError { message: String },

    /// Generic plugin error
    Generic { message: String },
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::PluginNotFound { plugin_name } => {
                write!(f, "Plugin not found: {}", plugin_name)
            }
            PluginError::VersionIncompatible { message } => {
                write!(f, "Version incompatible: {}", message)
            }
            PluginError::LoadError { plugin_name, cause } => {
                write!(f, "Failed to load plugin '{}': {}", plugin_name, cause)
            }
            PluginError::ExecutionError {
                plugin_name,
                operation,
                cause,
            } => {
                write!(
                    f,
                    "Plugin '{}' failed during '{}': {}",
                    plugin_name, operation, cause
                )
            }
            PluginError::AsyncError { message } => {
                write!(f, "Async operation error: {}", message)
            }
            PluginError::Generic { message } => {
                write!(f, "Plugin error: {}", message)
            }
        }
    }
}

impl std::error::Error for PluginError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_not_found_error() {
        let error = PluginError::PluginNotFound {
            plugin_name: "test-plugin".to_string(),
        };

        assert_eq!(error.to_string(), "Plugin not found: test-plugin");
        assert_eq!(
            error,
            PluginError::PluginNotFound {
                plugin_name: "test-plugin".to_string()
            }
        );
    }

    #[test]
    fn test_version_incompatible_error() {
        let error = PluginError::VersionIncompatible {
            message: "Plugin requires API version 2025.0.0, but system has 2024.0.0".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Version incompatible: Plugin requires API version 2025.0.0, but system has 2024.0.0"
        );
    }

    #[test]
    fn test_load_error() {
        let error = PluginError::LoadError {
            plugin_name: "dump".to_string(),
            cause: "Missing required dependencies".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Failed to load plugin 'dump': Missing required dependencies"
        );
    }

    #[test]
    fn test_execution_error() {
        let error = PluginError::ExecutionError {
            plugin_name: "dump".to_string(),
            operation: "process_message".to_string(),
            cause: "Invalid message format".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Plugin 'dump' failed during 'process_message': Invalid message format"
        );
    }

    #[test]
    fn test_async_error() {
        let error = PluginError::AsyncError {
            message: "Tokio runtime unavailable".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Async operation error: Tokio runtime unavailable"
        );
    }

    #[test]
    fn test_generic_error() {
        let error = PluginError::Generic {
            message: "Unknown plugin system failure".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Plugin error: Unknown plugin system failure"
        );
    }

    #[test]
    fn test_plugin_result_type_alias() {
        let success: PluginResult<String> = Ok("success".to_string());
        let failure: PluginResult<String> = Err(PluginError::Generic {
            message: "test failure".to_string(),
        });

        assert!(success.is_ok());
        assert!(failure.is_err());

        match failure {
            Err(PluginError::Generic { message }) => {
                assert_eq!(message, "test failure");
            }
            _ => panic!("Expected Generic error"),
        }
    }

    #[test]
    fn test_error_cloning() {
        let original = PluginError::PluginNotFound {
            plugin_name: "test".to_string(),
        };
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn test_error_debug_formatting() {
        let error = PluginError::ExecutionError {
            plugin_name: "test".to_string(),
            operation: "init".to_string(),
            cause: "failed".to_string(),
        };

        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("ExecutionError"));
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("init"));
        assert!(debug_str.contains("failed"));
    }
}
