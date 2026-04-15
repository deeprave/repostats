//! Public API for the plugin system
//!
//! This module provides the complete public API for the plugin system.
//! External modules should import from here rather than directly from internal modules.
//! The plugin system provides trait-based interfaces for dynamic plugin loading,
//! version compatibility, and real-time notifications.

use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;
use toml::Table;

// Primary plugin trait
pub use crate::plugin::traits::{ConsumerPlugin, Plugin};

// Core plugin management
pub use crate::plugin::manager::PluginManager;

// Error handling
pub use crate::plugin::error::{PluginError, PluginResult};

// Argument parsing and configuration
pub use crate::plugin::args::PluginConfig;

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

/// Stable plugin facade for lifecycle and inspection operations.
#[derive(Clone, Copy, Debug, Default)]
pub struct PluginService;

/// Access the stable plugin facade.
pub fn plugin_service() -> PluginService {
    PluginService
}

impl PluginService {
    /// Initialize the global plugin service.
    pub async fn initialize(self) -> PluginResult<()> {
        let mut manager = PLUGIN_SERVICE.lock().await;
        manager.initialize().await
    }

    /// Configure plugin timeout for the global plugin service.
    pub async fn configure_plugin_timeout(self, timeout: std::time::Duration) -> PluginResult<()> {
        let mut manager = PLUGIN_SERVICE.lock().await;
        manager.configure_plugin_timeout(timeout)
    }

    /// Return the current plugin API version.
    #[allow(dead_code)]
    pub async fn api_version(self) -> u32 {
        let manager = PLUGIN_SERVICE.lock().await;
        manager.api_version()
    }

    /// Return the currently active plugins.
    pub async fn get_active_plugins(self) -> Vec<String> {
        let manager = PLUGIN_SERVICE.lock().await;
        manager.get_active_plugins().await
    }

    /// Return whether active plugins suppress progress display.
    pub async fn should_suppress_progress(self) -> bool {
        let manager = PLUGIN_SERVICE.lock().await;
        manager.should_suppress_progress().await
    }

    /// List discovered plugins, optionally filtering to active only.
    pub async fn list_plugins_with_filter(
        self,
        active_only: bool,
    ) -> Vec<crate::plugin::types::PluginInfo> {
        let manager = PLUGIN_SERVICE.lock().await;
        manager.list_plugins_with_filter(active_only).await
    }

    /// Discover plugins from the configured plugin directories.
    pub async fn discover_plugins(
        self,
        plugin_dirs: &[String],
        exclusions: &[String],
    ) -> PluginResult<()> {
        let mut manager = PLUGIN_SERVICE.lock().await;
        manager.discover_plugins(plugin_dirs, exclusions).await
    }

    /// Apply plugin configuration extracted from the main TOML config.
    pub async fn set_plugin_configs(self, main_config: &Table) -> PluginResult<()> {
        let mut manager = PLUGIN_SERVICE.lock().await;
        manager.set_plugin_configs(main_config)
    }

    /// Activate the requested plugin set.
    pub async fn activate_plugins(
        self,
        command_segments: &[crate::app::cli::segmenter::CommandSegment],
        use_colors: bool,
    ) -> PluginResult<()> {
        let mut manager = PLUGIN_SERVICE.lock().await;
        manager.activate_plugins(command_segments, use_colors).await
    }

    /// Run active-plugin initialization compatibility steps.
    pub async fn initialize_active_plugins(self) -> PluginResult<()> {
        let mut manager = PLUGIN_SERVICE.lock().await;
        manager.initialize_active_plugins().await
    }

    /// Set up per-plugin notification subscribers.
    pub async fn setup_plugin_notification_subscribers(self) -> PluginResult<()> {
        let mut manager = PLUGIN_SERVICE.lock().await;
        manager.setup_plugin_notification_subscribers().await
    }

    /// Set up system-event subscription for plugin lifecycle coordination.
    pub async fn setup_system_notification_subscriber(self) -> PluginResult<()> {
        let mut manager = PLUGIN_SERVICE.lock().await;
        manager.setup_system_notification_subscriber().await
    }

    /// Return combined scan requirements for the active plugin set.
    pub async fn combined_requirements(self) -> crate::scanner::types::ScanRequires {
        let manager = PLUGIN_SERVICE.lock().await;
        manager.get_combined_requirements().await
    }
}

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
#[allow(dead_code)]
pub async fn get_plugin_service() -> tokio::sync::MutexGuard<'static, PluginManager> {
    let guard = PLUGIN_SERVICE.lock().await;
    guard
}
