//! Plugin Error Handling
//!
//! Comprehensive error types for plugin operations including loading, execution,
//! compatibility checking, and runtime failures.

use crate::core::error_handling::ContextualError;
use std::fmt;

/// Result type alias for plugin operations
pub type PluginResult<T> = std::result::Result<T, PluginError>;

/// Comprehensive error types for plugin system operations
#[derive(Debug)]
pub enum PluginError {
    /// Plugin not found in registry
    PluginNotFound { plugin_name: String },
    /// Initialization error
    PluginInitializationError { plugin_name: String, cause: String },
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

    /// Configuration error
    ConfigurationError {
        plugin_name: String,
        message: String,
    },

    /// IO operation error
    IoError {
        operation: String,
        path: String,
        cause: String,
    },

    /// Generic plugin error
    Generic { message: String },

    /// Wrapped error from another system
    Error {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
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
            PluginError::PluginInitializationError { plugin_name, cause } => {
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
            PluginError::ConfigurationError {
                plugin_name,
                message,
            } => {
                write!(
                    f,
                    "Configuration error in plugin '{}': {}",
                    plugin_name, message
                )
            }
            PluginError::IoError {
                operation,
                path,
                cause,
            } => {
                write!(f, "IO error during {} on '{}': {}", operation, path, cause)
            }
            PluginError::Generic { message } => {
                write!(f, "{}", message)
            }
            PluginError::Error { source } => {
                write!(f, "{}", source)
            }
        }
    }
}

impl std::error::Error for PluginError {}

impl From<crate::notifications::error::NotificationError> for PluginError {
    fn from(error: crate::notifications::error::NotificationError) -> Self {
        PluginError::Error {
            source: Box::new(error),
        }
    }
}

impl ContextualError for PluginError {
    fn is_user_actionable(&self) -> bool {
        match self {
            // Clear user-actionable errors that users can fix
            PluginError::Generic { .. } => true,
            PluginError::VersionIncompatible { .. } => true,
            PluginError::PluginNotFound { .. } => true,
            PluginError::PluginInitializationError { .. } => true,

            // User configuration errors
            PluginError::ConfigurationError { .. } => true,
            PluginError::IoError { .. } => true,

            // System/internal errors that users cannot directly fix
            PluginError::LoadError { .. }
            | PluginError::ExecutionError { .. }
            | PluginError::AsyncError { .. }
            | PluginError::Error { .. } => false,
        }
    }

    fn user_message(&self) -> Option<&str> {
        match self {
            // User-actionable errors with specific messages
            PluginError::Generic { message } => Some(message),
            PluginError::VersionIncompatible { message } => Some(message),

            // PluginNotFound shows a helpful message to guide users
            PluginError::PluginNotFound { plugin_name: _ } => {
                Some("Plugin not found. Check your plugin directory or plugin name spelling.")
            }

            // Configuration errors with specific messages
            PluginError::ConfigurationError { message, .. } => Some(message),
            PluginError::IoError { cause, .. } => Some(cause),

            // System errors - let generic context handle them
            _ => None,
        }
    }
}
