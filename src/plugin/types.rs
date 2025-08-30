//! Type definitions for the plugin system
//!
//! This module contains the core data structures used throughout
//! the plugin system for metadata, configuration, and plugin management.

use std::path::PathBuf;

/// Plugin metadata information
#[derive(Debug, Clone, PartialEq)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub api_version: u32,
}

/// Plugin function metadata
#[derive(Debug, Clone, PartialEq)]
pub struct PluginFunction {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
}

/// Plugin type classification
#[derive(Debug, Clone, PartialEq)]
pub enum PluginType {
    Processing,
    Output,
    Notification,
}

/// Information about an active plugin (matched to a command segment)
#[derive(Debug, Clone)]
pub struct ActivePluginInfo {
    /// Name of the plugin
    pub plugin_name: String,
    /// Function name that was matched
    pub function_name: String,
    /// Arguments for this plugin from the command segment
    pub args: Vec<String>,
}

/// Unique identifier for a plugin within the manager
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginId(pub u64);

impl PluginId {
    pub fn new(id: u64) -> Self {
        PluginId(id)
    }

    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

/// Plugin metadata exposed to external systems for display/help
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub functions: Vec<PluginFunction>,
    pub requires_file_content: bool,
    pub requires_historical_content: bool,
}

/// Simplified plugin proxy for controlled access
#[derive(Debug, Clone)]
pub struct PluginProxy {
    /// Plugin metadata
    pub metadata: PluginMetadata,
}

impl PluginProxy {
    /// Get plugin metadata for display/help systems
    pub fn get_metadata(&self) -> crate::plugin::error::PluginResult<PluginMetadata> {
        Ok(self.metadata.clone())
    }

    /// Configure plugin with command-line arguments (placeholder)
    pub fn parse_arguments(&self, _args: &[String]) -> crate::plugin::error::PluginResult<()> {
        // TODO: Implement argument parsing through PluginManager reference
        Ok(())
    }
}

/// Discovery result with plugin metadata and loading mechanism
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub info: PluginInfo,
    pub source: PluginSource,
    pub manifest_path: Option<PathBuf>,
}

/// Source of a discovered plugin
#[derive(Debug, Clone)]
pub enum PluginSource {
    /// External shared library plugin
    External { library_path: PathBuf },
    /// Built-in plugin factory
    Builtin {
        factory: fn() -> Box<dyn crate::plugin::traits::Plugin>,
    },
    /// Consumer plugin factory
    BuiltinConsumer {
        factory: fn() -> Box<dyn crate::plugin::traits::ConsumerPlugin>,
    },
}
