//! Public API for the plugin system
//!
//! This module provides the complete public API for the plugin system.
//! External modules should import from here rather than directly from internal modules.
//! The plugin system provides trait-based interfaces for dynamic plugin loading,
//! version compatibility, and real-time notifications.

// Core plugin management
pub use crate::plugin::manager::PluginManager;

// Error handling
pub use crate::plugin::error::{PluginError, PluginResult};

// Plugin traits and core interfaces
pub use crate::plugin::traits::{ConsumerPlugin, Plugin};

// Plugin metadata and information
pub use crate::plugin::types::{
    PluginFunction, PluginInfo, PluginMetadata, PluginProxy, PluginType,
};

// Plugin configuration and arguments
pub use crate::plugin::args::{OutputFormat, PluginArgParser, PluginConfig};

// Plugin registry for management
pub use crate::plugin::registry::{PluginRegistry, SharedPluginRegistry};

// Plugin discovery and loading
pub use crate::plugin::discovery::{DiscoveredPlugin, PluginSource};

// Plugin context and settings
pub use crate::plugin::context::PluginContext;
pub use crate::plugin::settings::PluginSettings;
