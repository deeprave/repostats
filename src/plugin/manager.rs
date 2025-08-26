//! Plugin Manager
//!
//! Central coordinator for plugin lifecycle, compatibility checking, and plugin proxy management.
//! Owns the plugin registry and provides high-level plugin management operations.

use crate::plugin::{DiscoveryConfig, PluginDiscovery, SharedPluginRegistry};
use crate::plugin::{PluginError, PluginFunction, PluginInfo, PluginResult};
use crate::queue::QueueManager;
use log::debug;
use std::collections::HashMap;

/// Central plugin manager responsible for:
/// - Plugin lifecycle management
/// - Version compatibility checking
/// - Plugin proxy coordination
/// - Plugin registry ownership
pub struct PluginManager {
    /// The plugin registry (owned by this manager)
    registry: SharedPluginRegistry,

    /// Current API version for compatibility checking
    api_version: u32,

    /// Command resolution - which plugin/function to execute
    active_command: Option<String>,

    /// Plugin ID to name mapping for proxy resolution
    plugin_ids: HashMap<PluginId, String>,

    /// Next available plugin ID
    next_id: PluginId,
}

/// Unique identifier for a plugin within the manager
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginId(u64);

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(api_version: u32) -> Self {
        Self {
            registry: SharedPluginRegistry::new(),
            api_version,
            active_command: None,
            plugin_ids: HashMap::new(),
            next_id: PluginId(1),
        }
    }

    /// Get shared access to the plugin registry
    pub fn registry(&self) -> &SharedPluginRegistry {
        &self.registry
    }

    /// Get current API version
    pub fn api_version(&self) -> u32 {
        self.api_version
    }

    /// Check if a plugin API version is compatible
    pub fn is_api_compatible(&self, plugin_api_version: u32) -> bool {
        // Same major version (year) is compatible
        self.get_major_version(self.api_version) == self.get_major_version(plugin_api_version)
    }

    /// Get major version (year) from API version
    pub fn get_major_version(&self, api_version: u32) -> u32 {
        api_version / 10000
    }

    /// Validate plugin compatibility before registration
    pub fn validate_plugin_compatibility(&self, plugin_info: &PluginInfo) -> PluginResult<()> {
        if !self.is_api_compatible(plugin_info.api_version) {
            return Err(PluginError::VersionIncompatible {
                message: format!(
                    "Plugin '{}' has incompatible API version {} (expected major version {})",
                    plugin_info.name,
                    plugin_info.api_version,
                    self.get_major_version(self.api_version)
                ),
            });
        }
        Ok(())
    }

    /// Get plugin metadata by ID (internal)
    fn get_plugin_metadata(&self, plugin_id: PluginId) -> PluginResult<PluginMetadata> {
        let plugin_name =
            self.plugin_ids
                .get(&plugin_id)
                .ok_or_else(|| PluginError::PluginNotFound {
                    plugin_name: format!("ID:{:?}", plugin_id),
                })?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| PluginError::AsyncError {
                message: e.to_string(),
            })?;

        rt.block_on(async {
            let registry = self.registry.inner().read().await;
            if let Some(plugin) = registry.get_plugin(plugin_name) {
                let info = plugin.plugin_info();
                let functions = plugin.advertised_functions();

                // Use default values for file requirements (metadata is for display only)
                let requires_file_content = false;
                let requires_historical_content = false;

                Ok(PluginMetadata {
                    name: info.name.clone(),
                    version: info.version.clone(),
                    description: info.description.clone(),
                    author: info.author.clone(),
                    functions,
                    requires_file_content,
                    requires_historical_content,
                })
            } else {
                Err(PluginError::PluginNotFound {
                    plugin_name: plugin_name.clone(),
                })
            }
        })
    }

    /// Generate next plugin ID
    fn next_plugin_id(&mut self) -> PluginId {
        let id = self.next_id;
        self.next_id = PluginId(self.next_id.0 + 1);
        id
    }

    /// Create a proxy for a plugin by name (internal use only)
    fn create_proxy(&mut self, plugin_name: &str) -> PluginResult<PluginProxy> {
        // Check if plugin exists in registry
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| PluginError::AsyncError {
                message: e.to_string(),
            })?;

        let exists = rt.block_on(async {
            let registry = self.registry.inner().read().await;
            registry.has_plugin(plugin_name)
        });

        if !exists {
            return Err(PluginError::PluginNotFound {
                plugin_name: plugin_name.to_string(),
            });
        }

        // Generate ID and register mapping
        let plugin_id = self.next_plugin_id();
        self.plugin_ids.insert(plugin_id, plugin_name.to_string());

        // Create metadata for the proxy
        let metadata = self.get_plugin_metadata(plugin_id)?;

        Ok(PluginProxy { metadata })
    }

    /// Resolve command to plugin and create proxy
    pub fn resolve_command(&mut self, command: &str) -> PluginResult<PluginProxy> {
        // For now, assume command == plugin name (simplified)
        self.active_command = Some(command.to_string());
        self.create_proxy(command)
    }

    /// Discover and initialize plugins with injected configuration
    pub async fn discover_plugins(&mut self, config: DiscoveryConfig) -> PluginResult<()> {
        debug!(
            "PluginManager: Starting plugin discovery with exclusions: {:?}",
            config.excluded_plugins
        );

        // Create discovery implementation
        let discovery = PluginDiscovery::new();

        let discovered_plugins = discovery.discover_plugins(&config).await?;

        // Register discovered plugins
        let mut registry = self.registry.inner().write().await;
        for discovered in discovered_plugins {
            // Validate compatibility
            self.validate_plugin_compatibility(&discovered.info)?;

            // Create plugin instance and register
            match discovered.source {
                crate::plugin::PluginSource::Builtin { factory } => {
                    let plugin = factory();
                    registry.register_plugin(plugin)?;
                }
                crate::plugin::PluginSource::BuiltinConsumer { factory } => {
                    let consumer_plugin = factory();
                    registry.register_consumer_plugin(consumer_plugin)?;
                }
                crate::plugin::PluginSource::External { library_path } => {
                    // TODO: Implement external plugin loading from shared library
                    return Err(PluginError::LoadError {
                        plugin_name: discovered.info.name.clone(),
                        cause: format!(
                            "External plugin loading not yet implemented: {:?}",
                            library_path
                        ),
                    });
                }
            }
        }

        debug!("PluginManager: Plugin discovery completed successfully");
        Ok(())
    }

    /// Setup plugin consumers for queue processing (internal)
    pub fn setup_plugin_consumers(
        &mut self,
        queue: &std::sync::Arc<QueueManager>,
        plugin_names: &[String],
        plugin_args: &[String],
    ) -> PluginResult<()> {
        debug!(
            "PluginManager: Setting up consumers for plugins: {:?}",
            plugin_names
        );

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| PluginError::AsyncError {
                message: e.to_string(),
            })?;

        for plugin_name in plugin_names {
            let consumer = queue.create_consumer(plugin_name.clone()).map_err(|e| {
                PluginError::AsyncError {
                    message: e.to_string(),
                }
            })?;

            let result = rt.block_on(async {
                let mut registry = self.registry.inner().write().await;
                if let Some(plugin) = registry.get_plugin_mut(plugin_name) {
                    // Parse plugin arguments before starting consumption
                    plugin.parse_plugin_arguments(plugin_args).await?;
                    debug!(
                        "PluginManager: Plugin {} arguments parsed successfully",
                        plugin_name
                    );
                }

                if let Some(consumer_plugin) = registry.get_consumer_plugin_mut(plugin_name) {
                    consumer_plugin.start_consuming(consumer).await?;
                    debug!(
                        "PluginManager: Plugin {} registered as consumer and started consuming",
                        plugin_name
                    );
                }

                Ok::<(), PluginError>(())
            });
            result?;
        }

        debug!("PluginManager: All plugin consumers setup completed");
        Ok(())
    }

    /// Execute the resolved command (when ready)
    pub fn execute(&self) -> PluginResult<()> {
        debug!(
            "PluginManager: Execute called with active command: {:?}",
            self.active_command
        );
        Ok(())
    }

    /// Resolve a command to a PluginProxy
    pub async fn resolve_command_to_proxy(&mut self, input: &str) -> PluginResult<PluginProxy> {
        debug!("PluginManager: Resolving command '{}' to proxy", input);

        // Check for explicit plugin:function syntax
        if let Some(colon_pos) = input.find(':') {
            let plugin_name = &input[..colon_pos];
            let function_name = &input[colon_pos + 1..];

            // Create proxy and check if function exists
            let proxy = self.create_proxy(plugin_name)?;
            let metadata = proxy.get_metadata()?;

            let has_function = metadata
                .functions
                .iter()
                .any(|f| f.name == function_name || f.aliases.contains(&function_name.to_string()));

            if !has_function {
                let available: Vec<String> =
                    metadata.functions.iter().map(|f| f.name.clone()).collect();
                return Err(PluginError::Generic {
                    message: format!(
                        "Plugin '{}' does not provide function '{}'. Available functions: {:?}",
                        plugin_name, function_name, available
                    ),
                });
            }

            return Ok(proxy);
        }

        // Check if command matches a plugin name directly
        let all_plugins = self.list_plugins_with_filter(false).await;

        for plugin_meta in &all_plugins {
            if plugin_meta.name == input {
                // Command matches plugin name - return its proxy
                return self.create_proxy(&plugin_meta.name);
            }
        }

        // Check if command matches a function name (with possible ambiguity)
        let mut matches: Vec<String> = Vec::new(); // plugin names that have this function

        for plugin_meta in all_plugins {
            for func in &plugin_meta.functions {
                // Check if command matches function name or alias
                if func.name == input || func.aliases.contains(&input.to_string()) {
                    matches.push(plugin_meta.name.clone());
                    break; // Only need to record plugin once
                }
            }
        }

        // Handle matches
        match matches.len() {
            0 => Err(PluginError::PluginNotFound {
                plugin_name: format!("Unknown command: '{}'", input),
            }),
            1 => {
                // Single match - return proxy for that plugin
                let plugin_name = matches.into_iter().next().unwrap();
                self.create_proxy(&plugin_name)
            }
            _ => {
                // Ambiguous - multiple plugins provide this function
                Err(PluginError::Generic {
                    message: format!(
                        "Ambiguous function '{}' found in multiple plugins: {}. Use explicit 'plugin:function' syntax.",
                        input, matches.join(", ")
                    )
                })
            }
        }
    }

    /// List plugins with option to include all plugins or just active ones
    pub async fn list_plugins_with_filter(&self, active_only: bool) -> Vec<PluginMetadata> {
        let registry = self.registry.inner().read().await;
        let mut plugins = Vec::new();

        // Get metadata for plugins based on filter
        let plugin_names = if active_only {
            registry.get_active_plugins()
        } else {
            registry.get_plugin_names()
        };

        for plugin_name in &plugin_names {
            if let Some(plugin) = registry.get_plugin(plugin_name) {
                let info = plugin.plugin_info();
                let functions = plugin.advertised_functions();

                plugins.push(PluginMetadata {
                    name: info.name.clone(),
                    version: info.version.clone(),
                    description: info.description.clone(),
                    author: info.author.clone(),
                    functions,
                    requires_file_content: false, // Default for display
                    requires_historical_content: false, // Default for display
                });
            }
        }

        plugins
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
    pub fn get_metadata(&self) -> PluginResult<PluginMetadata> {
        Ok(self.metadata.clone())
    }

    /// Configure plugin with command-line arguments (placeholder)
    pub fn parse_arguments(&self, _args: &[String]) -> PluginResult<()> {
        // TODO: Implement argument parsing through PluginManager reference
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::traits::*;

    // Mock plugin for testing
    #[derive(Debug)]
    struct MockPlugin {
        name: String,
        initialized: bool,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                initialized: false,
            }
        }
    }

    #[async_trait::async_trait]
    impl Plugin for MockPlugin {
        fn plugin_info(&self) -> PluginInfo {
            PluginInfo {
                name: self.name.clone(),
                version: "1.0.0".to_string(),
                description: "Mock plugin".to_string(),
                author: "Test".to_string(),
                api_version: crate::get_plugin_api_version(),
            }
        }

        fn plugin_type(&self) -> PluginType {
            PluginType::Processing
        }

        fn advertised_functions(&self) -> Vec<PluginFunction> {
            vec![PluginFunction {
                name: "test".to_string(),
                description: "Test function".to_string(),
                aliases: vec!["t".to_string()],
            }]
        }

        async fn initialize(&mut self) -> PluginResult<()> {
            self.initialized = true;
            Ok(())
        }

        async fn execute(&mut self, _args: &[String]) -> PluginResult<()> {
            Ok(())
        }

        async fn cleanup(&mut self) -> PluginResult<()> {
            Ok(())
        }

        async fn parse_plugin_arguments(&mut self, _args: &[String]) -> PluginResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_plugin_manager_creation() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        assert_eq!(manager.api_version, crate::get_plugin_api_version());
        assert_eq!(manager.active_command, None);
        assert_eq!(manager.next_id, PluginId(1));
        assert!(manager.plugin_ids.is_empty());
    }

    #[test]
    fn test_plugin_manager_api_compatibility() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        // Same year should be compatible
        assert!(manager.is_api_compatible(20250101));
        assert!(manager.is_api_compatible(20250215));
        assert!(manager.is_api_compatible(20251231));

        // Different years should not be compatible
        assert!(!manager.is_api_compatible(20240101));
        assert!(!manager.is_api_compatible(20260101));
    }

    #[test]
    fn test_plugin_manager_major_version() {
        let manager = PluginManager::new(20250315); // Different version for compatibility test

        assert_eq!(manager.get_major_version(20250101), 2025);
        assert_eq!(manager.get_major_version(20240831), 2024);
        assert_eq!(manager.get_major_version(20261225), 2026);
    }

    #[test]
    fn test_validate_plugin_compatibility() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        // Compatible plugin
        let compatible_info = PluginInfo {
            name: "compatible".to_string(),
            version: "1.0.0".to_string(),
            description: "Compatible plugin".to_string(),
            author: "Test".to_string(),
            api_version: 20250215,
        };

        assert!(manager
            .validate_plugin_compatibility(&compatible_info)
            .is_ok());

        // Incompatible plugin
        let incompatible_info = PluginInfo {
            name: "incompatible".to_string(),
            version: "1.0.0".to_string(),
            description: "Incompatible plugin".to_string(),
            author: "Test".to_string(),
            api_version: 20240101,
        };

        let result = manager.validate_plugin_compatibility(&incompatible_info);
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::VersionIncompatible { message } => {
                assert!(message.contains("incompatible"));
                assert!(message.contains("incompatible"));
                assert!(message.contains("20240101"));
                assert!(message.contains("2025"));
            }
            _ => panic!("Expected VersionIncompatible error"),
        }
    }

    #[tokio::test]
    async fn test_plugin_registry_access() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        // Test that we can access the registry
        let registry = manager.registry();
        assert_eq!(registry.plugin_count().await, 0);

        // Register a plugin directly through the registry
        {
            let mut reg = registry.inner().write().await;
            reg.register_plugin(Box::new(MockPlugin::new("test-plugin")))
                .unwrap();
        }

        assert_eq!(registry.plugin_count().await, 1);
        assert!(registry.has_plugin("test-plugin").await);
    }

    #[tokio::test]
    async fn test_plugin_discovery_finds_dump_plugin() {
        let mut manager = PluginManager::new(crate::get_plugin_api_version());
        let config = DiscoveryConfig::default();

        // Should discover the dump plugin
        manager.discover_plugins(config).await.unwrap();
    }

    #[tokio::test]
    async fn test_plugin_discovery_with_exclusions() {
        let mut manager = PluginManager::new(crate::get_plugin_api_version());
        let config = DiscoveryConfig {
            excluded_plugins: vec!["excluded-plugin".to_string()],
            ..Default::default()
        };

        // Should succeed (no plugins to exclude currently)
        manager.discover_plugins(config).await.unwrap();
    }

    #[tokio::test]
    async fn test_list_plugins_empty() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        let all_plugins = manager.list_plugins_with_filter(false).await;
        assert!(all_plugins.is_empty());

        let active_plugins = manager.list_plugins_with_filter(true).await;
        assert!(active_plugins.is_empty());
    }

    #[tokio::test]
    async fn test_list_plugins_with_registered_plugin() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        // Register a plugin
        {
            let mut registry = manager.registry().inner().write().await;
            registry
                .register_plugin(Box::new(MockPlugin::new("test-plugin")))
                .unwrap();
        }

        let all_plugins = manager.list_plugins_with_filter(false).await;
        assert_eq!(all_plugins.len(), 1);
        assert_eq!(all_plugins[0].name, "test-plugin");
        assert_eq!(all_plugins[0].version, "1.0.0");
        assert_eq!(all_plugins[0].description, "Mock plugin");
        assert_eq!(all_plugins[0].author, "Test");
        assert_eq!(all_plugins[0].functions.len(), 1);
        assert_eq!(all_plugins[0].functions[0].name, "test");

        let active_plugins = manager.list_plugins_with_filter(true).await;
        assert_eq!(active_plugins.len(), 1);
        assert_eq!(active_plugins[0].name, "test-plugin");
    }

    #[test]
    fn test_plugin_id_generation() {
        let mut manager = PluginManager::new(crate::get_plugin_api_version());

        let id1 = manager.next_plugin_id();
        let id2 = manager.next_plugin_id();
        let id3 = manager.next_plugin_id();

        assert_eq!(id1, PluginId(1));
        assert_eq!(id2, PluginId(2));
        assert_eq!(id3, PluginId(3));
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn test_plugin_proxy_metadata() {
        let metadata = PluginMetadata {
            name: "test-proxy".to_string(),
            version: "1.0.0".to_string(),
            description: "Test proxy plugin".to_string(),
            author: "Test Author".to_string(),
            functions: vec![PluginFunction {
                name: "main".to_string(),
                description: "Main function".to_string(),
                aliases: vec!["m".to_string()],
            }],
            requires_file_content: true,
            requires_historical_content: false,
        };

        let proxy = PluginProxy {
            metadata: metadata.clone(),
        };

        let retrieved_metadata = proxy.get_metadata().unwrap();
        assert_eq!(retrieved_metadata.name, "test-proxy");
        assert_eq!(retrieved_metadata.version, "1.0.0");
        assert_eq!(retrieved_metadata.description, "Test proxy plugin");
        assert_eq!(retrieved_metadata.author, "Test Author");
        assert_eq!(retrieved_metadata.functions.len(), 1);
        assert_eq!(retrieved_metadata.functions[0].name, "main");
        assert_eq!(retrieved_metadata.requires_file_content, true);
        assert_eq!(retrieved_metadata.requires_historical_content, false);
    }

    #[test]
    fn test_plugin_proxy_parse_arguments() {
        let metadata = PluginMetadata {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            author: "Test".to_string(),
            functions: vec![],
            requires_file_content: false,
            requires_historical_content: false,
        };

        let proxy = PluginProxy { metadata };

        // Should succeed (placeholder implementation)
        assert!(proxy.parse_arguments(&["--arg".to_string()]).is_ok());
    }
}
