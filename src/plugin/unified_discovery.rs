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

use crate::plugin::{PluginInfo, PluginResult};
use std::path::{Path, PathBuf};

/// Plugin discovery that handles all plugin types
pub struct PluginDiscovery {
    external_discovery: ExternalPluginDiscovery,
    builtin_discovery: BuiltinPluginDiscovery,
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
        factory: fn() -> Box<dyn crate::plugin::Plugin>,
    },
    /// Consumer plugin factory
    BuiltinConsumer {
        factory: fn() -> Box<dyn crate::plugin::ConsumerPlugin>,
    },
}

/// Configuration for plugin discovery
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Plugin directories to search (defaults to platform-specific paths)
    pub search_paths: Vec<PathBuf>,
    /// Plugins to exclude from discovery
    pub excluded_plugins: Vec<String>,
    /// Whether to include built-in plugins
    pub include_builtins: bool,
    /// Whether to include external plugins
    pub include_externals: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            search_paths: get_default_plugin_paths(),
            excluded_plugins: Vec::new(),
            include_builtins: true,
            include_externals: true,
        }
    }
}

/// External plugin discovery for cdylib binaries with YAML manifests
pub struct ExternalPluginDiscovery;

/// Built-in plugin discovery (minimal component)
pub struct BuiltinPluginDiscovery;

impl PluginDiscovery {
    /// Create new plugin discovery
    pub fn new() -> Self {
        Self {
            external_discovery: ExternalPluginDiscovery::new(),
            builtin_discovery: BuiltinPluginDiscovery::new(),
        }
    }

    /// Discover all available plugins
    pub async fn discover_plugins(
        &self,
        config: &DiscoveryConfig,
    ) -> PluginResult<Vec<DiscoveredPlugin>> {
        let mut plugins = Vec::new();

        // 1. Discover external plugins (main focus)
        if config.include_externals {
            let external_plugins = self
                .external_discovery
                .discover_external_plugins(config)
                .await?;
            plugins.extend(external_plugins);
        }

        // 2. Discover builtin plugins (small part)
        if config.include_builtins {
            let builtin_plugins = self.builtin_discovery.discover_builtin_plugins().await?;
            plugins.extend(builtin_plugins);
        }

        // 3. Apply exclusions
        plugins.retain(|plugin| !config.excluded_plugins.contains(&plugin.info.name));

        Ok(plugins)
    }
}

impl ExternalPluginDiscovery {
    pub fn new() -> Self {
        Self
    }

    /// Discover external plugin libraries with YAML manifests
    pub async fn discover_external_plugins(
        &self,
        config: &DiscoveryConfig,
    ) -> PluginResult<Vec<DiscoveredPlugin>> {
        let mut plugins = Vec::new();

        for search_path in &config.search_paths {
            if !search_path.exists() {
                continue;
            }

            let path_plugins = self.scan_plugin_directory(search_path).await?;
            plugins.extend(path_plugins);
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
    pub fn new() -> Self {
        Self
    }

    /// Discover built-in plugins (minimal set)
    pub async fn discover_builtin_plugins(&self) -> PluginResult<Vec<DiscoveredPlugin>> {
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

/// Get platform-specific default plugin search paths using dirs library
fn get_default_plugin_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // User-specific plugin directory
    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("repostats").join("plugins"));
    }

    // System-wide plugin directory
    if let Some(data_dir) = dirs::data_dir() {
        paths.push(data_dir.join("repostats").join("plugins"));
    }

    // Local plugins directory
    paths.push(PathBuf::from("./plugins"));

    paths
}

impl Default for PluginDiscovery {
    fn default() -> Self {
        Self::new()
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
        let _discovery = PluginDiscovery::new();
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
        let discovery = PluginDiscovery::new();
        let config = DiscoveryConfig {
            include_builtins: true,
            include_externals: false,
            ..Default::default()
        };

        let plugins = discovery.discover_plugins(&config).await.unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].info.name, "dump");
    }

    #[tokio::test]
    async fn test_discovery_config_defaults() {
        let config = DiscoveryConfig::default();
        assert!(config.include_builtins);
        assert!(config.include_externals);
        assert!(config.search_paths.len() >= 2); // Should have at least user and local paths
    }

    #[test]
    fn test_default_plugin_paths() {
        let paths = get_default_plugin_paths();
        assert!(!paths.is_empty());

        // Should include local plugins directory
        assert!(paths.iter().any(|p| p.ends_with("plugins")));
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
        let discovery = PluginDiscovery::new();
        let config = DiscoveryConfig {
            excluded_plugins: vec!["dump".to_string()],
            ..Default::default()
        };

        let plugins = discovery.discover_plugins(&config).await.unwrap();
        assert!(plugins.is_empty()); // dump should be filtered out
    }
}
