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

// Plugin metadata and information
pub use crate::plugin::types::PluginInfo;

// Plugin configuration and arguments

// Plugin registry for management
pub use crate::plugin::registry::{PluginRegistry, SharedPluginRegistry};

// Plugin discovery and loading

// Plugin context and settings
