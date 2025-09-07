//! Public API for the plugin system
//!
//! This module provides the complete public API for the plugin system.
//! External modules should import from here rather than directly from internal modules.
//! The plugin system provides trait-based interfaces for dynamic plugin loading,
//! version compatibility, and real-time notifications.

use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;

// Core plugin management
pub use crate::plugin::manager::PluginManager;

// Note: Direct plugin utilities have been moved to their respective modules
// Use crate::plugin::error::PluginError for error handling
// Use crate::plugin::events:: for plugin event publishing
// Use crate::plugin::types::PluginInfo for plugin metadata
// See plugin/manager.rs for core management functionality

/// Global plugin service instance
static PLUGIN_SERVICE: LazyLock<Arc<Mutex<PluginManager>>> = LazyLock::new(|| {
    log::trace!("Initializing plugin service");
    let plugin_manager = PluginManager::new(crate::core::version::get_api_version());
    log::trace!("Plugin service initialized successfully");
    Arc::new(Mutex::new(plugin_manager))
});

/// Access plugin service
///
/// Returns a reference to the global plugin service. This service manages
/// all plugins including discovery, loading, and lifecycle management.
/// Each call returns the same shared instance.
///
/// # Examples
/// ```no_run
/// # use repostats::plugin::api::get_plugin_service;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let plugin_manager = get_plugin_service().await;
/// let plugins = plugin_manager.list_plugins_with_filter(false).await;
/// # Ok(())
/// # }
/// ```
pub async fn get_plugin_service() -> tokio::sync::MutexGuard<'static, PluginManager> {
    log::trace!("Acquiring plugin service lock");
    let guard = PLUGIN_SERVICE.lock().await;
    log::trace!("Acquired plugin service lock");
    guard
}
