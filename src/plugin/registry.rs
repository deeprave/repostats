//! Plugin Registry
//!
//! Thread-safe plugin registry for managing loaded plugins with registration,
//! retrieval, and lifecycle management capabilities.

use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::traits::{ConsumerPlugin, Plugin};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Plugin registry for managing loaded plugins
pub struct PluginRegistry {
    /// Map of plugin name to plugin instance
    plugins: HashMap<String, Box<dyn Plugin>>,

    /// Map of plugin name to consumer plugin instance
    consumer_plugins: HashMap<String, Box<dyn ConsumerPlugin>>,

    /// Set of plugin names that are currently active
    active_plugins: HashSet<String>,
}

impl std::fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRegistry")
            .field("plugins", &self.plugins.keys().collect::<Vec<_>>())
            .field(
                "consumer_plugins",
                &self.consumer_plugins.keys().collect::<Vec<_>>(),
            )
            .field("active_plugins", &self.active_plugins)
            .finish()
    }
}

impl PluginRegistry {
    /// Create a new empty plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            consumer_plugins: HashMap::new(),
            active_plugins: HashSet::new(),
        }
    }

    /// Register a plugin in the registry
    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> PluginResult<()> {
        let plugin_name = plugin.plugin_info().name.clone();

        // Check if plugin name is already registered
        if self.plugins.contains_key(&plugin_name) {
            return Err(PluginError::Generic {
                message: format!("Plugin '{}' is already registered", plugin_name),
            });
        }

        self.plugins.insert(plugin_name, plugin);
        Ok(())
    }

    /// Register a consumer plugin in the registry
    pub fn register_consumer_plugin(
        &mut self,
        plugin: Box<dyn ConsumerPlugin>,
    ) -> PluginResult<()> {
        let plugin_name = plugin.plugin_info().name.clone();

        // Check if plugin name is already registered
        if self.consumer_plugins.contains_key(&plugin_name)
            || self.plugins.contains_key(&plugin_name)
        {
            return Err(PluginError::Generic {
                message: format!("Plugin '{}' is already registered", plugin_name),
            });
        }

        self.consumer_plugins.insert(plugin_name, plugin);
        Ok(())
    }

    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    /// Get a mutable plugin by name
    pub fn get_plugin_mut(&mut self, name: &str) -> Option<&mut Box<dyn Plugin>> {
        self.plugins.get_mut(name)
    }

    /// Get a consumer plugin by name
    pub fn get_consumer_plugin(&self, name: &str) -> Option<&dyn ConsumerPlugin> {
        self.consumer_plugins.get(name).map(|p| p.as_ref())
    }

    /// Get a mutable consumer plugin by name
    pub fn get_consumer_plugin_mut(&mut self, name: &str) -> Option<&mut Box<dyn ConsumerPlugin>> {
        self.consumer_plugins.get_mut(name)
    }

    /// Check if a plugin exists in the registry
    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugins.contains_key(name) || self.consumer_plugins.contains_key(name)
    }

    /// Get list of all plugin names
    pub fn get_plugin_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        names.extend(self.plugins.keys().cloned());
        names.extend(self.consumer_plugins.keys().cloned());
        names.sort();
        names
    }

    /// Get list of active plugin names (only truly active plugins)
    pub fn get_active_plugins(&self) -> Vec<String> {
        let mut active: Vec<String> = self.active_plugins.iter().cloned().collect();
        active.sort();
        active
    }

    /// Activate a plugin (mark it as active)
    pub fn activate_plugin(&mut self, name: &str) -> PluginResult<()> {
        if !self.has_plugin(name) {
            return Err(PluginError::PluginNotFound {
                plugin_name: name.to_string(),
            });
        }
        self.active_plugins.insert(name.to_string());
        Ok(())
    }

    /// Deactivate a plugin (mark it as inactive)
    pub fn deactivate_plugin(&mut self, name: &str) -> PluginResult<()> {
        if !self.has_plugin(name) {
            return Err(PluginError::PluginNotFound {
                plugin_name: name.to_string(),
            });
        }
        self.active_plugins.remove(name);
        Ok(())
    }

    /// Check if a plugin is currently active
    pub fn is_plugin_active(&self, name: &str) -> bool {
        self.active_plugins.contains(name)
    }

    /// Clear all active plugins
    pub fn clear_active_plugins(&mut self) {
        self.active_plugins.clear();
    }

    /// Remove a plugin from the registry
    pub fn unregister_plugin(&mut self, name: &str) -> PluginResult<()> {
        let removed =
            self.plugins.remove(name).is_some() || self.consumer_plugins.remove(name).is_some();

        if !removed {
            return Err(PluginError::PluginNotFound {
                plugin_name: name.to_string(),
            });
        }

        // Also remove from active plugins
        self.active_plugins.remove(name);
        Ok(())
    }

    /// Get total count of registered plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len() + self.consumer_plugins.len()
    }

    /// Clear all plugins from registry
    pub fn clear(&mut self) {
        self.plugins.clear();
        self.consumer_plugins.clear();
        self.active_plugins.clear();
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe shared plugin registry
#[derive(Debug, Clone)]
pub struct SharedPluginRegistry {
    inner: Arc<RwLock<PluginRegistry>>,
}

impl SharedPluginRegistry {
    /// Create a new shared plugin registry
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(PluginRegistry::new())),
        }
    }

    /// Get access to the inner registry for read/write operations
    pub fn inner(&self) -> &Arc<RwLock<PluginRegistry>> {
        &self.inner
    }

    /// Convenience method to check if plugin exists
    pub async fn has_plugin(&self, name: &str) -> bool {
        let registry = self.inner.read().await;
        registry.has_plugin(name)
    }

    /// Convenience method to get plugin names
    pub async fn get_plugin_names(&self) -> Vec<String> {
        let registry = self.inner.read().await;
        registry.get_plugin_names()
    }

    /// Convenience method to get plugin count
    pub async fn plugin_count(&self) -> usize {
        let registry = self.inner.read().await;
        registry.plugin_count()
    }

    /// Convenience method to get active plugin names
    pub async fn get_active_plugins(&self) -> Vec<String> {
        let registry = self.inner.read().await;
        registry.get_active_plugins()
    }

    /// Convenience method to activate a plugin
    pub async fn activate_plugin(&self, name: &str) -> PluginResult<()> {
        let mut registry = self.inner.write().await;
        registry.activate_plugin(name)
    }

    /// Convenience method to deactivate a plugin
    pub async fn deactivate_plugin(&self, name: &str) -> PluginResult<()> {
        let mut registry = self.inner.write().await;
        registry.deactivate_plugin(name)
    }

    /// Convenience method to check if plugin is active
    pub async fn is_plugin_active(&self, name: &str) -> bool {
        let registry = self.inner.read().await;
        registry.is_plugin_active(name)
    }

    /// Convenience method to clear all active plugins
    pub async fn clear_active_plugins(&self) {
        let mut registry = self.inner.write().await;
        registry.clear_active_plugins();
    }
}

impl Default for SharedPluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::args::PluginConfig;
    use crate::plugin::types::{PluginFunction, PluginInfo, PluginType};
    use crate::queue::api::QueueConsumer;

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
                api_version: 20250101,
                plugin_type: self.plugin_type(),
                functions: self.advertised_functions(),
                required: self.requirements(),
                auto_active: false,
            }
        }

        fn plugin_type(&self) -> PluginType {
            PluginType::Processing
        }

        fn advertised_functions(&self) -> Vec<PluginFunction> {
            vec![]
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

        async fn parse_plugin_arguments(
            &mut self,
            _args: &[String],
            _config: &PluginConfig,
        ) -> PluginResult<()> {
            Ok(())
        }
    }

    // Mock consumer plugin for testing
    #[derive(Debug)]
    struct MockConsumerPlugin {
        base: MockPlugin,
        consuming: bool,
    }

    impl MockConsumerPlugin {
        fn new(name: &str) -> Self {
            Self {
                base: MockPlugin::new(name),
                consuming: false,
            }
        }
    }

    #[async_trait::async_trait]
    impl Plugin for MockConsumerPlugin {
        fn plugin_info(&self) -> PluginInfo {
            self.base.plugin_info()
        }

        fn plugin_type(&self) -> PluginType {
            PluginType::Output
        }

        fn advertised_functions(&self) -> Vec<PluginFunction> {
            self.base.advertised_functions()
        }

        async fn initialize(&mut self) -> PluginResult<()> {
            self.base.initialize().await
        }

        async fn execute(&mut self, args: &[String]) -> PluginResult<()> {
            self.base.execute(args).await
        }

        async fn cleanup(&mut self) -> PluginResult<()> {
            self.consuming = false;
            self.base.cleanup().await
        }

        async fn parse_plugin_arguments(
            &mut self,
            args: &[String],
            config: &PluginConfig,
        ) -> PluginResult<()> {
            self.base.parse_plugin_arguments(args, config).await
        }
    }

    #[async_trait::async_trait]
    impl ConsumerPlugin for MockConsumerPlugin {
        async fn start_consuming(&mut self, _consumer: QueueConsumer) -> PluginResult<()> {
            self.consuming = true;
            Ok(())
        }

        async fn stop_consuming(&mut self) -> PluginResult<()> {
            self.consuming = false;
            Ok(())
        }
    }

    #[test]
    fn test_plugin_registry_creation() {
        let registry = PluginRegistry::new();

        assert_eq!(registry.plugin_count(), 0);
        assert!(registry.get_plugin_names().is_empty());
        assert!(registry.get_active_plugins().is_empty());
    }

    #[test]
    fn test_plugin_registry_default() {
        let registry = PluginRegistry::default();

        assert_eq!(registry.plugin_count(), 0);
    }

    #[test]
    fn test_plugin_registration() {
        let mut registry = PluginRegistry::new();
        let plugin = Box::new(MockPlugin::new("test-plugin"));

        // Register plugin
        registry.register_plugin(plugin).unwrap();

        assert_eq!(registry.plugin_count(), 1);
        assert!(registry.has_plugin("test-plugin"));
        assert!(!registry.has_plugin("nonexistent"));

        let plugin_names = registry.get_plugin_names();
        assert_eq!(plugin_names, vec!["test-plugin"]);
    }

    #[test]
    fn test_consumer_plugin_registration() {
        let mut registry = PluginRegistry::new();
        let consumer_plugin = Box::new(MockConsumerPlugin::new("consumer-plugin"));

        // Register consumer plugin
        registry.register_consumer_plugin(consumer_plugin).unwrap();

        assert_eq!(registry.plugin_count(), 1);
        assert!(registry.has_plugin("consumer-plugin"));

        let plugin_names = registry.get_plugin_names();
        assert_eq!(plugin_names, vec!["consumer-plugin"]);
    }

    #[test]
    fn test_duplicate_plugin_registration() {
        let mut registry = PluginRegistry::new();

        let plugin1 = Box::new(MockPlugin::new("duplicate"));
        let plugin2 = Box::new(MockPlugin::new("duplicate"));

        // First registration should succeed
        registry.register_plugin(plugin1).unwrap();

        // Second registration should fail
        let result = registry.register_plugin(plugin2);
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::Generic { message } => {
                assert!(message.contains("already registered"));
                assert!(message.contains("duplicate"));
            }
            _ => panic!("Expected Generic error"),
        }
    }

    #[test]
    fn test_plugin_retrieval() {
        let mut registry = PluginRegistry::new();
        let plugin = Box::new(MockPlugin::new("retrieval-test"));

        registry.register_plugin(plugin).unwrap();

        // Test get_plugin
        let retrieved = registry.get_plugin("retrieval-test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().plugin_info().name, "retrieval-test");

        // Test get_plugin_mut
        let retrieved_mut = registry.get_plugin_mut("retrieval-test");
        assert!(retrieved_mut.is_some());
        assert_eq!(retrieved_mut.unwrap().plugin_info().name, "retrieval-test");

        // Test nonexistent plugin
        assert!(registry.get_plugin("nonexistent").is_none());
        assert!(registry.get_plugin_mut("nonexistent").is_none());
    }

    #[test]
    fn test_consumer_plugin_retrieval() {
        let mut registry = PluginRegistry::new();
        let consumer_plugin = Box::new(MockConsumerPlugin::new("consumer-test"));

        registry.register_consumer_plugin(consumer_plugin).unwrap();

        // Test get_consumer_plugin
        let retrieved = registry.get_consumer_plugin("consumer-test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().plugin_info().name, "consumer-test");

        // Test get_consumer_plugin_mut
        let retrieved_mut = registry.get_consumer_plugin_mut("consumer-test");
        assert!(retrieved_mut.is_some());
        assert_eq!(retrieved_mut.unwrap().plugin_info().name, "consumer-test");

        // Test nonexistent plugin
        assert!(registry.get_consumer_plugin("nonexistent").is_none());
        assert!(registry.get_consumer_plugin_mut("nonexistent").is_none());
    }

    #[test]
    fn test_plugin_unregistration() {
        let mut registry = PluginRegistry::new();
        let plugin = Box::new(MockPlugin::new("unregister-test"));

        registry.register_plugin(plugin).unwrap();
        assert!(registry.has_plugin("unregister-test"));

        // Unregister plugin
        registry.unregister_plugin("unregister-test").unwrap();
        assert!(!registry.has_plugin("unregister-test"));
        assert_eq!(registry.plugin_count(), 0);

        // Try to unregister nonexistent plugin
        let result = registry.unregister_plugin("nonexistent");
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::PluginNotFound { plugin_name } => {
                assert_eq!(plugin_name, "nonexistent");
            }
            _ => panic!("Expected PluginNotFound error"),
        }
    }

    #[test]
    fn test_plugin_registry_clear() {
        let mut registry = PluginRegistry::new();

        // Register multiple plugins
        registry
            .register_plugin(Box::new(MockPlugin::new("plugin1")))
            .unwrap();
        registry
            .register_plugin(Box::new(MockPlugin::new("plugin2")))
            .unwrap();
        registry
            .register_consumer_plugin(Box::new(MockConsumerPlugin::new("consumer1")))
            .unwrap();

        assert_eq!(registry.plugin_count(), 3);

        // Clear registry
        registry.clear();
        assert_eq!(registry.plugin_count(), 0);
        assert!(registry.get_plugin_names().is_empty());
    }

    #[test]
    fn test_multiple_plugin_types() {
        let mut registry = PluginRegistry::new();

        // Register both regular and consumer plugins
        registry
            .register_plugin(Box::new(MockPlugin::new("regular1")))
            .unwrap();
        registry
            .register_plugin(Box::new(MockPlugin::new("regular2")))
            .unwrap();
        registry
            .register_consumer_plugin(Box::new(MockConsumerPlugin::new("consumer1")))
            .unwrap();
        registry
            .register_consumer_plugin(Box::new(MockConsumerPlugin::new("consumer2")))
            .unwrap();

        assert_eq!(registry.plugin_count(), 4);

        let mut plugin_names = registry.get_plugin_names();
        plugin_names.sort();
        assert_eq!(
            plugin_names,
            vec!["consumer1", "consumer2", "regular1", "regular2"]
        );

        // Test has_plugin works for both types
        assert!(registry.has_plugin("regular1"));
        assert!(registry.has_plugin("consumer1"));

        // Test active plugins (should be empty since none are activated)
        let active_names = registry.get_active_plugins();
        assert!(active_names.is_empty());

        // Test activation of specific plugins
        registry.activate_plugin("regular1").unwrap();
        registry.activate_plugin("consumer1").unwrap();

        let mut active_names = registry.get_active_plugins();
        active_names.sort();
        assert_eq!(active_names, vec!["consumer1", "regular1"]);

        // Test deactivation
        registry.deactivate_plugin("regular1").unwrap();
        let active_names = registry.get_active_plugins();
        assert_eq!(active_names, vec!["consumer1"]);
    }

    #[tokio::test]
    async fn test_shared_plugin_registry_creation() {
        let shared_registry = SharedPluginRegistry::new();

        assert_eq!(shared_registry.plugin_count().await, 0);
        assert!(shared_registry.get_plugin_names().await.is_empty());
    }

    #[tokio::test]
    async fn test_shared_plugin_registry_default() {
        let shared_registry = SharedPluginRegistry::default();

        assert_eq!(shared_registry.plugin_count().await, 0);
    }

    #[tokio::test]
    async fn test_shared_plugin_registry_thread_safety() {
        let shared_registry = SharedPluginRegistry::new();

        // Register plugin through write lock
        {
            let mut registry = shared_registry.inner().write().await;
            registry
                .register_plugin(Box::new(MockPlugin::new("thread-safe")))
                .unwrap();
        }

        // Check through convenience methods
        assert!(shared_registry.has_plugin("thread-safe").await);
        assert_eq!(shared_registry.plugin_count().await, 1);

        let plugin_names = shared_registry.get_plugin_names().await;
        assert_eq!(plugin_names, vec!["thread-safe"]);
    }

    #[tokio::test]
    async fn test_shared_plugin_registry_concurrent_access() {
        use tokio::task;

        let shared_registry = SharedPluginRegistry::new();

        // Spawn multiple tasks to register plugins concurrently
        let tasks: Vec<_> = (0..5)
            .map(|i| {
                let registry = shared_registry.clone();
                task::spawn(async move {
                    let mut reg = registry.inner().write().await;
                    reg.register_plugin(Box::new(MockPlugin::new(&format!("concurrent-{}", i))))
                        .unwrap();
                    i
                })
            })
            .collect();

        // Wait for all tasks to complete
        for task in tasks {
            task.await.unwrap();
        }

        // All 5 plugins should be registered
        assert_eq!(shared_registry.plugin_count().await, 5);

        let plugin_names = shared_registry.get_plugin_names().await;
        assert_eq!(plugin_names.len(), 5);

        // Check specific plugins exist
        for i in 0..5 {
            assert!(
                shared_registry
                    .has_plugin(&format!("concurrent-{}", i))
                    .await
            );
        }
    }

    #[test]
    fn test_plugin_activation_errors() {
        let mut registry = PluginRegistry::new();

        // Try to activate non-existent plugin
        let result = registry.activate_plugin("nonexistent");
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::PluginNotFound { plugin_name } => {
                assert_eq!(plugin_name, "nonexistent");
            }
            _ => panic!("Expected PluginNotFound error"),
        }

        // Try to deactivate non-existent plugin
        let result = registry.deactivate_plugin("nonexistent");
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::PluginNotFound { plugin_name } => {
                assert_eq!(plugin_name, "nonexistent");
            }
            _ => panic!("Expected PluginNotFound error"),
        }

        // Test is_plugin_active for non-existent plugin
        assert!(!registry.is_plugin_active("nonexistent"));

        // Register a plugin and test activation
        registry
            .register_plugin(Box::new(MockPlugin::new("test")))
            .unwrap();
        assert!(!registry.is_plugin_active("test"));

        registry.activate_plugin("test").unwrap();
        assert!(registry.is_plugin_active("test"));

        registry.deactivate_plugin("test").unwrap();
        assert!(!registry.is_plugin_active("test"));
    }

    #[test]
    fn test_registration_vs_activation_separation() {
        let mut registry = PluginRegistry::new();

        // Register multiple plugins
        registry
            .register_plugin(Box::new(MockPlugin::new("plugin1")))
            .unwrap();
        registry
            .register_plugin(Box::new(MockPlugin::new("plugin2")))
            .unwrap();
        registry
            .register_consumer_plugin(Box::new(MockConsumerPlugin::new("consumer1")))
            .unwrap();

        // Verify all are registered
        assert_eq!(registry.plugin_count(), 3);
        assert!(registry.has_plugin("plugin1"));
        assert!(registry.has_plugin("plugin2"));
        assert!(registry.has_plugin("consumer1"));

        // CRITICAL: Verify NO plugins are active after registration
        let active = registry.get_active_plugins();
        assert!(
            active.is_empty(),
            "No plugins should be active immediately after registration"
        );

        // Explicitly activate only some plugins
        registry.activate_plugin("plugin1").unwrap();
        registry.activate_plugin("consumer1").unwrap();

        // Verify only activated plugins are active
        let mut active = registry.get_active_plugins();
        active.sort();
        assert_eq!(active, vec!["consumer1", "plugin1"]);

        // Verify plugin2 is registered but NOT active
        assert!(registry.has_plugin("plugin2"));
        assert!(!registry.is_plugin_active("plugin2"));
    }

    #[test]
    fn test_cross_type_name_collision() {
        let mut registry = PluginRegistry::new();

        // Register regular plugin first
        registry
            .register_plugin(Box::new(MockPlugin::new("collision")))
            .unwrap();

        // Try to register consumer plugin with same name - should fail
        let result =
            registry.register_consumer_plugin(Box::new(MockConsumerPlugin::new("collision")));
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::Generic { message } => {
                assert!(message.contains("already registered"));
                assert!(message.contains("collision"));
            }
            _ => panic!("Expected Generic error"),
        }
    }
}
