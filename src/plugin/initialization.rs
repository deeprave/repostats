//! Plugin initialization helper module
//!
//! This module handles plugin initialization, including setting up notification
//! managers, parsing arguments, and injecting consumers for ConsumerPlugins.

use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::args::PluginConfig;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::registry::PluginRegistry;
use crate::plugin::traits::Plugin;
use crate::queue::api::QueueManager;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Helper struct for managing plugin initialization
pub struct PluginInitializer {
    /// Notification manager for plugins
    notification_manager: Arc<Mutex<AsyncNotificationManager>>,
    /// Whether to use colors in output
    use_colors: Option<bool>,
}

impl PluginInitializer {
    /// Create a new PluginInitializer
    pub fn new(
        notification_manager: Arc<Mutex<AsyncNotificationManager>>,
        use_colors: Option<bool>,
    ) -> Self {
        Self {
            notification_manager,
            use_colors,
        }
    }

    /// Initialize a single plugin with its configuration and arguments
    pub async fn initialize_plugin(
        &self,
        registry: &mut PluginRegistry,
        plugin_name: &str,
        args: &[String],
        plugin_toml_config: Option<&toml::Table>,
        queue: &Arc<QueueManager>,
    ) -> PluginResult<()> {
        // Create plugin config
        let plugin_config = if let Some(toml_table) = plugin_toml_config {
            PluginConfig::from_toml(self.use_colors, toml_table)
        } else {
            PluginConfig::default()
        };

        // Get the plugin from registry
        let plugin =
            registry
                .get_plugin_mut(plugin_name)
                .ok_or_else(|| PluginError::PluginNotFound {
                    plugin_name: plugin_name.to_string(),
                })?;

        // Set notification manager
        plugin.set_notification_manager(self.notification_manager.clone());

        // Initialize the plugin
        plugin
            .initialize()
            .await
            .map_err(|e| PluginError::ExecutionError {
                plugin_name: plugin_name.to_string(),
                operation: "initialize".to_string(),
                cause: format!("Failed to initialize plugin: {}", e),
            })?;

        // Parse plugin arguments
        plugin.parse_plugin_arguments(args, &plugin_config).await?;

        // Inject consumer if this is a ConsumerPlugin
        if let Some(consumer_plugin) = plugin.as_mut().as_consumer_plugin() {
            self.inject_consumer(consumer_plugin, plugin_name, queue)
                .await?;
        }

        Ok(())
    }

    /// Inject a consumer into a ConsumerPlugin
    async fn inject_consumer(
        &self,
        consumer_plugin: &mut dyn crate::plugin::traits::ConsumerPlugin,
        plugin_name: &str,
        queue: &Arc<QueueManager>,
    ) -> PluginResult<()> {
        // Create consumer for the plugin
        let consumer = queue
            .create_consumer(plugin_name.to_string())
            .map_err(|e| PluginError::AsyncError {
                message: format!(
                    "Failed to create consumer for plugin '{}': {}",
                    plugin_name, e
                ),
            })?;

        // Inject the consumer
        consumer_plugin
            .inject_consumer(consumer)
            .await
            .map_err(|e| PluginError::ExecutionError {
                plugin_name: plugin_name.to_string(),
                operation: "inject_consumer".to_string(),
                cause: format!("Failed to inject consumer: {}", e),
            })?;

        log::trace!(
            "PluginInitializer: Consumer injected into plugin '{}' during initialization",
            plugin_name
        );

        Ok(())
    }

    /// Initialize multiple plugins
    pub async fn initialize_plugins(
        &self,
        registry: &mut PluginRegistry,
        plugins: &[(String, Vec<String>)], // (plugin_name, args)
        plugin_configs: &std::collections::HashMap<String, toml::Table>,
        queue: &Arc<QueueManager>,
    ) -> PluginResult<()> {
        for (plugin_name, args) in plugins {
            let plugin_config = plugin_configs.get(plugin_name);

            self.initialize_plugin(registry, plugin_name, args, plugin_config, queue)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::api::AsyncNotificationManager;
    use crate::plugin::args::PluginConfig;
    use crate::plugin::error::{PluginError, PluginResult};
    use crate::plugin::traits::Plugin;
    use crate::plugin::types::{PluginFunction, PluginInfo, PluginType};
    use crate::scanner::types::ScanRequires;

    /// Simple mock plugin for testing
    #[derive(Debug)]
    struct MockPlugin {
        name: String,
        info: PluginInfo,
        initialized: bool,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                info: PluginInfo {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: "Mock plugin for testing".to_string(),
                    author: "Test Author".to_string(),
                    api_version: 20250101,
                    plugin_type: PluginType::Processing,
                    functions: vec![PluginFunction {
                        name: "test".to_string(),
                        description: "Test function".to_string(),
                        aliases: vec![],
                    }],
                    required: ScanRequires::NONE,
                    auto_active: false,
                },
                initialized: false,
            }
        }
    }

    #[async_trait::async_trait]
    impl Plugin for MockPlugin {
        fn plugin_info(&self) -> PluginInfo {
            self.info.clone()
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

        fn requirements(&self) -> ScanRequires {
            ScanRequires::NONE
        }

        fn set_notification_manager(&mut self, _manager: Arc<Mutex<AsyncNotificationManager>>) {}

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

        async fn parse_plugin_arguments(
            &mut self,
            _args: &[String],
            _config: &PluginConfig,
        ) -> PluginResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_plugin_initializer_creation() {
        let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
        let initializer = PluginInitializer::new(notification_manager, Some(false));

        assert!(initializer.use_colors == Some(false));
    }

    #[tokio::test]
    async fn test_initialize_plugin_not_found() {
        let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
        let initializer = PluginInitializer::new(notification_manager, None);

        let mut registry = PluginRegistry::new();
        let queue = Arc::new(crate::queue::api::QueueManager::new());

        let result = initializer
            .initialize_plugin(&mut registry, "nonexistent", &[], None, &queue)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::PluginNotFound { plugin_name } => {
                assert_eq!(plugin_name, "nonexistent");
            }
            _ => panic!("Expected PluginNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_initialize_multiple_plugins() {
        let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
        let initializer = PluginInitializer::new(notification_manager.clone(), None);

        let mut registry = PluginRegistry::new();

        // Register test plugins
        let plugin1 = MockPlugin::new("plugin1");
        let plugin2 = MockPlugin::new("plugin2");

        registry.register_plugin(Box::new(plugin1)).unwrap();
        registry.register_plugin(Box::new(plugin2)).unwrap();

        // Activate plugins
        registry.activate_plugin("plugin1").unwrap();
        registry.activate_plugin("plugin2").unwrap();

        let queue = Arc::new(crate::queue::api::QueueManager::new());
        let plugins = vec![
            ("plugin1".to_string(), vec![]),
            ("plugin2".to_string(), vec!["--arg".to_string()]),
        ];
        let configs = std::collections::HashMap::new();

        let result = initializer
            .initialize_plugins(&mut registry, &plugins, &configs, &queue)
            .await;

        assert!(result.is_ok());
    }
}
