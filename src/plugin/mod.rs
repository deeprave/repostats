//! Plugin System Module
//!
//! Provides a trait-based interface for plugin communication with async notifications.
//! Supports dynamic plugin loading, version compatibility, and real-time notifications.

pub mod builtin;
pub mod context;
pub mod discovery;
pub mod error;
pub mod manager;
pub mod registry;
pub mod settings;
pub mod traits;
pub mod unified_discovery;

// Re-export core types for easier access
pub use context::PluginContext;
pub use error::{PluginError, PluginResult};
pub use manager::{PluginManager, PluginMetadata, PluginProxy};
pub use registry::{PluginRegistry, SharedPluginRegistry};
pub use settings::PluginSettings;
pub use traits::{
    ConsumerPlugin, Plugin, PluginDataRequirements, PluginFunction, PluginInfo, PluginType,
};
pub use unified_discovery::{
    BuiltinPluginDiscovery, DiscoveredPlugin, DiscoveryConfig, ExternalPluginDiscovery,
    PluginDiscovery, PluginSource,
};
