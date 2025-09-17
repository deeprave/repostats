//! Plugin Error Handling
//!
//! Comprehensive error types for plugin operations including loading, execution,
//! compatibility checking, and runtime failures.

use crate::core::error_handling::ContextualError;
use thiserror::Error;

/// Result type alias for plugin operations
pub type PluginResult<T> = std::result::Result<T, PluginError>;

/// Comprehensive error types for plugin system operations
#[derive(Error, Debug)]
pub enum PluginError {
    /// Plugin not found in registry
    #[error("Plugin not found: {plugin_name}")]
    PluginNotFound { plugin_name: String },
    /// Initialization error
    #[error("Failed to initialize plugin '{plugin_name}': {cause}")]
    PluginInitializationError { plugin_name: String, cause: String },
    /// Plugin API version incompatible with system
    #[error("Version incompatible: {message}")]
    VersionIncompatible { message: String },

    /// Plugin failed to load or initialize
    #[error("Failed to load plugin '{plugin_name}': {cause}")]
    LoadError { plugin_name: String, cause: String },

    /// Plugin execution failed
    #[error("Plugin '{plugin_name}' failed during '{operation}': {cause}")]
    ExecutionError {
        plugin_name: String,
        operation: String,
        cause: String,
    },

    /// Async operation error
    #[error("Async operation error: {message}")]
    AsyncError { message: String },

    /// Configuration error
    #[error("Configuration error in plugin '{plugin_name}': {message}")]
    ConfigurationError {
        plugin_name: String,
        message: String,
    },

    /// IO operation error
    #[error("IO error during {operation} on '{path}'")]
    IoError {
        operation: String,
        path: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Generic plugin error
    #[error("{message}")]
    Generic { message: String },

    /// Wrapped notification error
    #[error("Notification system error")]
    NotificationError {
        #[from]
        #[source]
        source: crate::notifications::error::NotificationError,
    },

    /// Wrapped error from another system
    #[error(transparent)]
    Error {
        #[from]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl PluginError {
    /// Attempt to downcast this error to a concrete error type.
    ///
    /// This method allows recovery of the original error type from wrapped errors,
    /// enabling more specific error handling when needed.
    pub fn downcast_ref<T: std::error::Error + 'static>(&self) -> Option<&T> {
        use std::any::{Any, TypeId};

        match self {
            // Check if T is NotificationError and we have a NotificationError variant
            PluginError::NotificationError { source } => {
                if TypeId::of::<T>()
                    == TypeId::of::<crate::notifications::error::NotificationError>()
                {
                    // Safe cast since we verified the type
                    (source as &dyn Any).downcast_ref::<T>()
                } else {
                    None
                }
            }

            // Downcast from IoError's source
            PluginError::IoError {
                source: Some(source),
                ..
            } => source.downcast_ref::<T>(),

            // Downcast from generic Error variant
            PluginError::Error { source } => source.downcast_ref::<T>(),

            // For other variants, no source to downcast from
            _ => None,
        }
    }

    /// Attempt to downcast this error to a concrete error type, consuming self.
    ///
    /// This method allows recovery of the original error by consuming the PluginError,
    /// useful when you need owned access to the underlying error.
    pub fn downcast<T: std::error::Error + 'static>(self) -> Result<T, Self> {
        use std::any::TypeId;

        match self {
            // Check if T is NotificationError and we have a NotificationError variant
            PluginError::NotificationError { source } => {
                if TypeId::of::<T>()
                    == TypeId::of::<crate::notifications::error::NotificationError>()
                {
                    // We need to convert the NotificationError to T
                    // This is a bit tricky with Rust's type system, so we'll use unsafe here
                    // after verifying the types match
                    let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(source);
                    match boxed.downcast::<T>() {
                        Ok(downcasted) => Ok(*downcasted),
                        Err(original) => {
                            // This shouldn't happen since we checked TypeId, but handle gracefully
                            let notif_err = *original
                                .downcast::<crate::notifications::error::NotificationError>()
                                .expect("Type verification failed");
                            Err(PluginError::NotificationError { source: notif_err })
                        }
                    }
                } else {
                    Err(PluginError::NotificationError { source })
                }
            }

            // Downcast from generic Error variant
            PluginError::Error { source } => match source.downcast::<T>() {
                Ok(downcasted) => Ok(*downcasted),
                Err(original) => Err(PluginError::Error { source: original }),
            },

            // For other variants, return self unchanged
            other => Err(other),
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
            | PluginError::NotificationError { .. }
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
            PluginError::IoError { .. } => None, // IoError details are in the error message

            // System errors - let generic context handle them
            _ => None,
        }
    }
}
