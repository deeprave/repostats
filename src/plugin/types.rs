//! Type definitions for the plugin system
//!
//! This module contains the core data structures used throughout
//! the plugin system for metadata, configuration, and plugin management.

use crate::scanner::types::ScanRequires;

/// Plugin metadata information
#[derive(Debug, Clone, PartialEq)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub api_version: u32,
    pub plugin_type: PluginType,
    pub functions: Vec<String>,
    pub required: ScanRequires,
    pub auto_active: bool,
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
