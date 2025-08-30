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
                write!(f, "{}", message)
            }
        }
    }
}

impl std::error::Error for PluginError {}
