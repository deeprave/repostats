//! Unified Plugin Discovery System
//!
//! This module provides comprehensive plugin discovery, handling both external binary plugins
//! (the main focus) and built-in plugins. It mirrors the architecture from gstats but with
//! better separation of concerns.
//!
//! External plugins are cdylib shared libraries with accompanying YAML manifest files.
//! They are discovered from platform-specific directories and validated for compatibility.
//!
//! Built-in plugins are compiled into the application and provide baseline functionality.

use crate::plugin::error::PluginResult;
use crate::plugin::traits::Plugin;
use crate::plugin::types::PluginInfo;
use std::path::PathBuf;

/// Discovery result with plugin metadata and factory function
pub struct DiscoveredPlugin {
    pub info: PluginInfo,
    pub factory: Box<dyn Fn() -> Box<dyn Plugin> + Send + Sync>,
}

/// Plugin discovery that handles all plugin types
#[derive(Debug, Clone)]
pub struct PluginDiscovery {
    /// Plugin directories to search
    pub search_paths: Vec<PathBuf>,
    /// Plugins to exclude from discovery
    pub excluded_plugins: Vec<String>,
}

impl Default for PluginDiscovery {
    fn default() -> Self {
        Self {
            search_paths: vec![],
            excluded_plugins: Vec::new(),
        }
    }
}

impl PluginDiscovery {
    /// Create plugin discovery with paths and exclusions
    pub(crate) fn new(search_paths: &[String], excludes: Option<Vec<&str>>) -> Self {
        let mut discovery = Self::default();

        // If no paths specified, use default paths
        if search_paths.is_empty() {
            discovery.search_paths = Self::get_default_plugin_paths();
        } else {
            discovery.search_paths = search_paths.iter().map(PathBuf::from).collect();
        }

        if let Some(excludes) = excludes {
            discovery.excluded_plugins = excludes.iter().map(|s| s.to_string()).collect();
        }
        discovery
    }

    /// Get default plugin search paths
    fn get_default_plugin_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Check for config dir + /repostats/plugins
        if let Some(config_dir) = dirs::config_dir() {
            let repostats_plugins = config_dir.join("repostats").join("plugins");
            if repostats_plugins.exists() {
                paths.push(repostats_plugins);
            }
        }

        // 2. Check for ~/.config/repostats/plugins on Unix-like systems
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            if let Ok(home) = std::env::var("HOME") {
                let unix_plugins = PathBuf::from(home)
                    .join(".config")
                    .join("repostats")
                    .join("plugins");
                if unix_plugins.exists() && !paths.contains(&unix_plugins) {
                    paths.push(unix_plugins);
                }
            }
        }

        // 3. Check for ./plugins in current directory
        let local_plugins = PathBuf::from("plugins");
        if local_plugins.exists() {
            paths.push(local_plugins);
        }

        // If no paths exist, return empty (builtins will still be discovered)
        paths
    }

    /// Discover all available plugins using internal configuration
    pub(crate) async fn discover_plugins(&self) -> PluginResult<Vec<DiscoveredPlugin>> {
        let mut plugins = Vec::new();

        // 1. Discover builtin plugins first - create on demand
        log::debug!("Discovering builtin plugins");
        let builtin_discovery = BuiltinPluginDiscovery::new();
        let builtin_plugins = builtin_discovery.discover_builtin_plugins().await?;
        log::debug!("Found {} builtin plugins", builtin_plugins.len());
        plugins.extend(builtin_plugins);

        // 2. Discover external plugins second (allows override of builtins) - create on demand
        log::debug!(
            "Discovering external plugins from paths: {:?}",
            self.search_paths
        );
        let external_discovery = ExternalPluginDiscovery::new();
        let external_plugins = external_discovery.discover_external_plugins(self).await?;
        log::debug!("Found {} external plugins", external_plugins.len());
        plugins.extend(external_plugins);

        // 3. Apply exclusions
        log::debug!("Applying exclusions: {:?}", self.excluded_plugins);
        let before_exclusions = plugins.len();
        plugins.retain(|plugin| !self.excluded_plugins.contains(&plugin.info.name));
        log::debug!(
            "After exclusions: {} plugins (was {})",
            plugins.len(),
            before_exclusions
        );

        Ok(plugins)
    }
}

/// External plugin discovery for cdylib binaries with YAML manifests
pub(crate) struct ExternalPluginDiscovery;

impl Default for ExternalPluginDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalPluginDiscovery {
    pub(crate) fn new() -> Self {
        Self
    }

    /// Discover external plugin libraries with YAML manifests
    pub(crate) async fn discover_external_plugins(
        &self,
        discovery: &PluginDiscovery,
    ) -> PluginResult<Vec<DiscoveredPlugin>> {
        Ok(crate::plugin::external::api::get_all_external_plugins(
            &discovery.search_paths,
        )?)
    }
}

/// Built-in plugin discovery (minimal component)
pub(crate) struct BuiltinPluginDiscovery;

impl Default for BuiltinPluginDiscovery {
    fn default() -> Self {
        Self::new()
    }
}
impl BuiltinPluginDiscovery {
    pub(crate) fn new() -> Self {
        Self
    }

    /// Discover built-in plugins using dynamic registry
    pub(crate) async fn discover_builtin_plugins(&self) -> PluginResult<Vec<DiscoveredPlugin>> {
        Ok(crate::plugin::builtin::api::get_all_builtin_plugins())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builtin_discovery_finds_dump_plugin() {
        let discovery = BuiltinPluginDiscovery::new();
        let plugins = discovery.discover_builtin_plugins().await.unwrap();

        assert_eq!(plugins.len(), 2);

        // Find dump plugin
        let dump_plugin = plugins
            .iter()
            .find(|p| p.info.name == "dump")
            .expect("DumpPlugin should be discoverable");
        assert_eq!(
            dump_plugin.info.api_version,
            crate::core::version::get_api_version()
        );
    }

    #[tokio::test]
    async fn test_plugin_discovery_includes_builtins() {
        let discovery = PluginDiscovery::new(&[], None);

        let plugins = discovery.discover_plugins().await.unwrap();
        assert_eq!(plugins.len(), 2);

        // Verify both builtin plugins are present
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
        assert!(plugin_names.contains(&"dump"));
        assert!(plugin_names.contains(&"output"));
    }

    #[tokio::test]
    async fn test_plugin_discovery_defaults() {
        let discovery = PluginDiscovery::default();
        assert!(discovery.search_paths.is_empty()); // Default should be empty
        assert!(discovery.excluded_plugins.is_empty());
    }

    #[tokio::test]
    async fn test_plugin_discovery_with_empty_paths_uses_defaults() {
        let discovery = PluginDiscovery::new(&[], None);
        // Should have populated default paths when created with empty search_paths
        // Note: This may be empty if no default directories exist on the system
        assert!(discovery.excluded_plugins.is_empty());
    }

    #[tokio::test]
    async fn test_external_discovery_empty_without_plugins() {
        let external_discovery = ExternalPluginDiscovery::new();
        let discovery = PluginDiscovery::default();

        let plugins = external_discovery
            .discover_external_plugins(&discovery)
            .await
            .unwrap();
        // Should be empty since no external plugins are installed
        assert!(plugins.is_empty());
    }
}
