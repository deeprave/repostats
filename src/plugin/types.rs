//! Type definitions for the plugin system
//!
//! This module contains the core data structures used throughout
//! the plugin system for metadata, configuration, and plugin management.

use crate::scanner::types::ScanRequires;
use std::path::PathBuf;

/// Plugin metadata information
#[derive(Debug, Clone, PartialEq)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub api_version: u32,
    pub plugin_type: PluginType,
    pub functions: Vec<PluginFunction>,
    pub required: ScanRequires,
    pub auto_active: bool,
}

/// Plugin function metadata
#[derive(Debug, Clone, PartialEq)]
pub struct PluginFunction {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
}

/// Plugin type classification
///
/// Defines the functional category and queue interaction behavior of plugins:
///
/// - `Processing`: Plugins that actively consume and process queue messages.
///   This includes data transformation, analysis, debugging, and monitoring plugins.
///   Examples: statistical analyzers, format converters, debug dumpers.
///   **Always gets queue subscribers for message processing.**
///
/// - `Output`: Plugins that generate final reports, exports, or files.
///   These plugins typically work with processed data but don't need live queue access.
///   Examples: report generators, file exporters, summary creators.
///   **Does not get queue subscribers.**
///
/// - `Notification`: Event-driven plugins that respond to system notifications.
///   These plugins react to system events rather than processing message queues.
///   Examples: webhook notifiers, system monitors, health checkers.
///   **Does not get queue subscribers.**
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
