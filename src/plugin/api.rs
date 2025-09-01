//! Public API for the plugin system
//!
//! This module provides the complete public API for the plugin system.
//! External modules should import from here rather than directly from internal modules.
//! The plugin system provides trait-based interfaces for dynamic plugin loading,
//! version compatibility, and real-time notifications.

// Core plugin management
pub use crate::plugin::manager::PluginManager;

// Error handling
pub use crate::plugin::error::PluginError;
pub use crate::plugin::error_handling::log_plugin_error_with_context;

// Plugin metadata and information
pub use crate::plugin::types::PluginInfo;
