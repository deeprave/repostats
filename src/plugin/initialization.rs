//! Plugin initialization helper module
//!
//! This module handles plugin initialization, including setting up notification
//! managers, parsing arguments, and injecting consumers for ConsumerPlugins.

use crate::plugin::api::PluginManager;
use crate::plugin::args::PluginConfig;
use crate::plugin::error::{PluginError, PluginResult};
use crate::queue::api::QueueManager;
use std::collections::HashMap;
use std::sync::Arc;

/// Helper struct for managing plugin initialization
pub struct PluginInitializer<'a> {
    manager: &'a mut PluginManager,
    /// Whether to use colors in output
    use_colors: bool,
}

impl<'a> PluginInitializer<'a> {
    /// Create a new PluginInitializer
    pub fn new(manager: &'a mut PluginManager, use_colors: bool) -> Self {
        Self {
            manager,
            use_colors,
        }
    }

    /// Initialize a single plugin with its configuration and arguments
    pub async fn initialize_plugin(
        &self,
        plugin_name: &str,
        args: &[String],
        plugin_toml_config: Option<&toml::Table>,
        queue_manager: &Arc<QueueManager>,
    ) -> PluginResult<bool> {
        // Create plugin config
        let plugin_config = if let Some(toml_table) = plugin_toml_config {
            PluginConfig::from_toml(self.use_colors, toml_table)
        } else {
            PluginConfig {
                use_colors: self.use_colors,
                ..PluginConfig::default()
            }
        };

        // Get the plugin from registry
        {
            let mut registry = self.manager.registry().inner().write().await;
            let plugin = registry.get_plugin_mut(plugin_name).ok_or_else(|| {
                PluginError::PluginNotFound {
                    plugin_name: plugin_name.to_string(),
                }
            })?;

            // Set notification manager
            plugin.set_notification_manager(self.manager.notification_manager());
            // Initialize the plugin
            plugin
                .initialize()
                .await
                .map_err(|e| PluginError::ExecutionError {
                    plugin_name: plugin_name.to_string(),
                    operation: "initialize".to_string(),
                    cause: format!("Failed to initialize plugin: {}", e),
                })?;

            plugin.parse_plugin_arguments(&args, &plugin_config).await?;

            // Inject consumer if this is a ConsumerPlugin
            if let Some(consumer_plugin) = plugin.as_mut().as_consumer_plugin() {
                self.inject_consumer(consumer_plugin, plugin_name, queue_manager)
                    .await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
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
        plugins: &HashMap<String, Vec<String>>, // (plugin_name, args)
        plugin_configs: &HashMap<String, toml::Table>,
        queue_manager: &Arc<QueueManager>,
    ) -> PluginResult<i32> {
        let mut consumer_count = 0;
        for (plugin_name, args) in plugins {
            let plugin_config = plugin_configs.get(plugin_name);

            if self
                .initialize_plugin(plugin_name, args, plugin_config, queue_manager)
                .await?
            {
                consumer_count += 1;
            }
        }

        Ok(consumer_count)
    }

    pub async fn activate_plugins(&self, plugin_names: &Vec<String>) -> PluginResult<()> {
        for plugin_name in plugin_names {
            self.activate_plugin(plugin_name).await?;
        }
        Ok(())
    }

    pub async fn activate_plugin(&self, name: &str) -> PluginResult<()> {
        let mut registry = self.manager.registry().inner().write().await;
        registry.activate_plugin(name)
    }

    pub async fn execute_plugin(&self, plugin_name: &str) -> PluginResult<()> {
        // Execute the plugin using the execution token pattern
        self.manager.registry().execute_plugin(plugin_name).await
    }

    /// Execute multiple plugins
    pub async fn execute_active_plugins(&self) -> PluginResult<()> {
        let active_plugins = self.manager.get_active_plugins().await;
        for plugin_name in &active_plugins {
            self.execute_plugin(plugin_name)
                .await
                .map_err(|e| PluginError::ExecutionError {
                    plugin_name: plugin_name.to_string(),
                    operation: "execute_plugin".to_string(),
                    cause: format!("Failed to execute plugin: {}", e),
                })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::api::AsyncNotificationManager;
    use crate::plugin::args::PluginConfig;
    use crate::plugin::error::PluginResult;
    use crate::plugin::traits::Plugin;
    use crate::plugin::types::{PluginInfo, PluginType};
    use crate::scanner::types::ScanRequires;
    use tokio::sync::Mutex;

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
                    functions: vec!["test".to_string()],
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

        fn advertised_functions(&self) -> Vec<String> {
            vec!["test".to_string()]
        }

        fn requirements(&self) -> ScanRequires {
            ScanRequires::NONE
        }

        fn set_notification_manager(&mut self, _manager: Arc<Mutex<AsyncNotificationManager>>) {}

        async fn initialize(&mut self) -> PluginResult<()> {
            self.initialized = true;
            Ok(())
        }

        async fn execute(&mut self) -> PluginResult<()> {
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
}
