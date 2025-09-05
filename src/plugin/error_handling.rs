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
/// (Example usage removed – helper not exposed publicly.)
pub fn log_plugin_error_with_context(error: &PluginError, operation_context: &str) {
    log_error_with_context(error, operation_context);
}
