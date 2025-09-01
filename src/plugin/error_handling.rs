//! Plugin-specific error handling utilities
//!
//! Provides convenience functions for plugin error handling that delegate to
//! the generic error logging system in core. This maintains the plugin API
//! while using the shared error handling infrastructure.

use crate::core::error_handling::log_error_with_context;
use crate::plugin::error::PluginError;

/// Log plugin errors using the generic error logging system
///
/// This is a convenience wrapper around the generic `log_error_with_context`
/// function that maintains API compatibility for plugin error handling.
/// The actual error handling logic is implemented through the ContextualError
/// trait in the core error handling system.
///
/// # Arguments
/// * `error` - The plugin error to handle
/// * `operation_context` - Human-readable description of the operation that failed
///
/// # Examples
/// ```rust
/// use crate::plugin::error_handling::log_plugin_error_with_context;
/// use crate::plugin::error::PluginError;
///
/// // User-actionable error shows specific message
/// let err = PluginError::Generic { message: "Invalid argument '--foo'".to_string() };
/// log_plugin_error_with_context(&err, "Command line parsing failed");
/// // Logs: "FATAL: Invalid argument '--foo'"
///
/// // System error shows generic context with debug details
/// let err = PluginError::LoadError {
///     plugin_name: "test".to_string(),
///     cause: "Library not found".to_string()
/// };
/// log_plugin_error_with_context(&err, "Plugin loading failed");
/// // Logs: "FATAL: Plugin loading failed" + debug details
/// ```
pub fn log_plugin_error_with_context(error: &PluginError, operation_context: &str) {
    log_error_with_context(error, operation_context);
}
