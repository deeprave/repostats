//! Unified Plugin Discovery System
//!
//! This module provides comprehensive plugin discovery, handling both external binary plugins
//! (the main focus) and built-in plugins. It mirrors the architecture from gstats but with
//! better separation of concerns.
//!
//! ## External Plugins
//! External plugins are cdylib shared libraries with accompanying YAML manifest files.
//! They are discovered from platform-specific directories and validated for compatibility.
//!
//! ## Built-in Plugins
//! Built-in plugins are compiled into the application and provide baseline functionality.

use crate::plugin::error::PluginResult;
use crate::plugin::types::{DiscoveredPlugin, PluginInfo, PluginSource};
use std::path::{Path, PathBuf};

/// Plugin discovery that handles all plugin types
pub struct PluginDiscovery {
    config: DiscoveryConfig,
}

/// Configuration for plugin discovery
#[derive(Debug, Clone)]
pub(crate) struct DiscoveryConfig {
    /// Plugin directory to search (defaults to platform-specific path)
    pub search_path: Option<PathBuf>,
    /// Plugins to exclude from discovery
    pub excluded_plugins: Vec<String>,
    /// Whether to include built-in plugins (internal use)
    pub include_builtins: bool,
    /// Whether to include external plugins (internal use)
    pub include_externals: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            search_path: get_default_plugin_path(),
            excluded_plugins: Vec::new(),
            include_builtins: true,
            include_externals: true,
        }
    }
}

/// External plugin discovery for cdylib binaries with YAML manifests
pub(crate) struct ExternalPluginDiscovery;

/// Built-in plugin discovery (minimal component)
pub(crate) struct BuiltinPluginDiscovery;

impl PluginDiscovery {
    /// Create plugin discovery with specified search path
    pub(crate) fn with_path(search_path: Option<&str>) -> Self {
        let mut config = DiscoveryConfig::default();
        if let Some(path) = search_path {
            config.search_path = Some(PathBuf::from(path));
        }

        Self { config }
    }

    /// Create plugin discovery with exclusions
    pub(crate) fn with_excludes(excludes: Vec<&str>) -> Self {
        let mut config = DiscoveryConfig::default();
        config.excluded_plugins = excludes.iter().map(|s| s.to_string()).collect();

        Self { config }
    }

    /// Create plugin discovery with path and exclusions
    pub(crate) fn with_path_and_excludes(search_path: Option<&str>, excludes: Vec<&str>) -> Self {
        let mut config = DiscoveryConfig::default();
        if let Some(path) = search_path {
            config.search_path = Some(PathBuf::from(path));
        }
        config.excluded_plugins = excludes.iter().map(|s| s.to_string()).collect();

        Self { config }
    }

    /// Internal method to control plugin type inclusion
    pub(crate) fn with_inclusion_config(
        search_path: Option<&str>,
        excludes: Vec<&str>,
        include_builtins: bool,
        include_externals: bool,
    ) -> Self {
        let mut config = DiscoveryConfig::default();
        if let Some(path) = search_path {
            config.search_path = Some(PathBuf::from(path));
        }
        config.excluded_plugins = excludes.iter().map(|s| s.to_string()).collect();
        config.include_builtins = include_builtins;
        config.include_externals = include_externals;

        Self { config }
    }

    /// Discover all available plugins using internal configuration
    pub(crate) async fn discover_plugins(&self) -> PluginResult<Vec<DiscoveredPlugin>> {
        let mut plugins = Vec::new();

        // 1. Discover external plugins (main focus) - create on demand
        if self.config.include_externals {
            log::debug!(
                "Discovering external plugins from path: {:?}",
                self.config.search_path
            );
            let external_discovery = ExternalPluginDiscovery::new();
            let external_plugins = external_discovery
                .discover_external_plugins(&self.config)
                .await?;
            log::debug!("Found {} external plugins", external_plugins.len());
            plugins.extend(external_plugins);
        }

        // 2. Discover builtin plugins (small part) - create on demand
        if self.config.include_builtins {
            log::debug!("Discovering builtin plugins");
            let builtin_discovery = BuiltinPluginDiscovery::new();
            let builtin_plugins = builtin_discovery.discover_builtin_plugins().await?;
            log::debug!("Found {} builtin plugins", builtin_plugins.len());
            plugins.extend(builtin_plugins);
        }

        // 3. Apply exclusions
        log::debug!("Applying exclusions: {:?}", self.config.excluded_plugins);
        let before_exclusions = plugins.len();
        plugins.retain(|plugin| !self.config.excluded_plugins.contains(&plugin.info.name));
        log::debug!(
            "After exclusions: {} plugins (was {})",
            plugins.len(),
            before_exclusions
        );

        Ok(plugins)
    }
}

impl ExternalPluginDiscovery {
    pub(crate) fn new() -> Self {
        Self
    }

    /// Discover external plugin libraries with YAML manifests
    pub(crate) async fn discover_external_plugins(
        &self,
        config: &DiscoveryConfig,
    ) -> PluginResult<Vec<DiscoveredPlugin>> {
        let mut plugins = Vec::new();

        if let Some(search_path) = &config.search_path {
            if search_path.exists() {
                let path_plugins = self.scan_plugin_directory(search_path).await?;
                plugins.extend(path_plugins);
            }
        }

        Ok(plugins)
    }

    /// Scan a directory for plugin pairs (cdylib + YAML)
    async fn scan_plugin_directory(&self, _dir: &Path) -> PluginResult<Vec<DiscoveredPlugin>> {
        let plugins = Vec::new();

        // TODO: Implement directory scanning for:
        // 1. Find .so/.dylib/.dll files
        // 2. Look for corresponding .yml/.yaml manifest files
        // 3. Parse manifest files for plugin metadata
        // 4. Validate plugin compatibility
        // 5. Create DiscoveredPlugin entries

        // Placeholder implementation
        Ok(plugins)
    }
}

impl BuiltinPluginDiscovery {
    pub(crate) fn new() -> Self {
        Self
    }

    /// Discover built-in plugins (minimal set)
    pub(crate) async fn discover_builtin_plugins(&self) -> PluginResult<Vec<DiscoveredPlugin>> {
        use crate::plugin::builtin::dump::DumpPlugin;

        let plugins = vec![DiscoveredPlugin {
            info: PluginInfo {
                name: "dump".to_string(),
                version: "1.0.0".to_string(),
                description: "Output queue messages to stdout for debugging".to_string(),
                author: "RepoStats".to_string(),
                api_version: crate::get_plugin_api_version(),
            },
            source: PluginSource::BuiltinConsumer {
                factory: || Box::new(DumpPlugin::new()),
            },
            manifest_path: None,
        }];

        Ok(plugins)
    }
}

/// Get platform-specific default plugin search path using dirs library
pub(crate) fn get_default_plugin_path() -> Option<PathBuf> {
    // User-specific plugin directory (preferred)
    if let Some(config_dir) = dirs::config_dir() {
        return Some(config_dir.join("Repostats"));
    }

    // Fallback to local plugins directory
    Some(PathBuf::from("./plugins"))
}

impl Default for PluginDiscovery {
    fn default() -> Self {
        Self::with_path(None)
    }
}

impl Default for ExternalPluginDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for BuiltinPluginDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_discovery_creation() {
        let _discovery = PluginDiscovery::with_path(None);
        assert!(true); // Basic creation test
    }

    #[tokio::test]
    async fn test_builtin_discovery_finds_dump_plugin() {
        let discovery = BuiltinPluginDiscovery::new();
        let plugins = discovery.discover_builtin_plugins().await.unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].info.name, "dump");
        assert_eq!(plugins[0].info.api_version, crate::get_plugin_api_version());
    }

    #[tokio::test]
    async fn test_plugin_discovery_includes_builtins() {
        let mut discovery = PluginDiscovery::with_inclusion_config(
            None,
            vec![],
            true,  // include_builtins
            false, // include_externals
        );

        let plugins = discovery.discover_plugins().await.unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].info.name, "dump");
    }

    #[tokio::test]
    async fn test_discovery_config_defaults() {
        let config = DiscoveryConfig::default();
        assert!(config.include_builtins);
        assert!(config.include_externals);
        assert!(config.search_path.is_some()); // Should have a default path
    }

    #[test]
    fn test_default_plugin_path() {
        let path = get_default_plugin_path();
        assert!(path.is_some());

        let path = path.unwrap();
        // Should be either config dir with "Repostats" or local plugins
        assert!(path.ends_with("Repostats") || path.ends_with("plugins"));
    }

    #[tokio::test]
    async fn test_external_discovery_empty_without_plugins() {
        let discovery = ExternalPluginDiscovery::new();
        let config = DiscoveryConfig::default();

        let plugins = discovery.discover_external_plugins(&config).await.unwrap();
        // Should be empty since no external plugins are installed
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_exclusions_filter_plugins() {
        let mut discovery = PluginDiscovery::with_excludes(vec!["dump"]);

        let plugins = discovery.discover_plugins().await.unwrap();
        assert!(plugins.is_empty()); // dump should be filtered out
    }
}
