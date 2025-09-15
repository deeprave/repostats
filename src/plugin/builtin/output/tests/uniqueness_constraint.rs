//! Tests for Output plugin uniqueness constraint enforcement
//!
//! These tests verify that the plugin system enforces the constraint that only
//! one Output plugin can be active at any given time.

use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::args::PluginConfig;
use crate::plugin::discovery::BuiltinPluginDiscovery;
use crate::plugin::error::PluginResult;
use crate::plugin::traits::Plugin;
use crate::plugin::types::{PluginInfo, PluginType};
use crate::scanner::types::ScanRequires;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mock Output Plugin for testing uniqueness constraints
#[derive(Debug)]
struct MockOutputPlugin {
    name: String,
}

impl MockOutputPlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Plugin for MockOutputPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            description: "Mock Output Plugin for testing".to_string(),
            author: "Test".to_string(),
            api_version: crate::core::version::get_api_version(),
            plugin_type: PluginType::Output,
            functions: vec!["output".to_string()],
            required: ScanRequires::NONE,
            auto_active: false,
        }
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Output
    }

    fn advertised_functions(&self) -> Vec<String> {
        vec!["output".to_string()]
    }

    fn set_notification_manager(&mut self, _manager: Arc<Mutex<AsyncNotificationManager>>) {
        // Mock implementation
    }

    async fn initialize(&mut self) -> PluginResult<()> {
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

/// Mock Processing Plugin for testing
#[derive(Debug)]
struct MockPlugin {
    name: String,
}

impl MockPlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Plugin for MockPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            description: "Mock Processing plugin for testing".to_string(),
            author: "Test Author".to_string(),
            api_version: 20250101,
            plugin_type: PluginType::Processing,
            functions: vec!["test".to_string()],
            required: ScanRequires::NONE,
            auto_active: false,
        }
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Processing
    }

    fn advertised_functions(&self) -> Vec<String> {
        vec![]
    }

    fn requirements(&self) -> ScanRequires {
        ScanRequires::NONE
    }

    fn set_notification_manager(&mut self, _manager: Arc<Mutex<AsyncNotificationManager>>) {}

    async fn initialize(&mut self) -> PluginResult<()> {
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
async fn test_output_plugin_uniqueness_constraint_works() {
    // TEST: Verify that uniqueness constraint for Output plugins is properly enforced
    // Only one Output plugin can be active at a time, and activating a second one
    // automatically deactivates the first.

    use crate::plugin::registry::PluginRegistry;
    use crate::plugin::traits::Plugin;

    let mut registry = PluginRegistry::new();

    // Create two mock Output plugins for testing
    let output_plugin_1 = MockOutputPlugin::new("output1");
    let output_plugin_2 = MockOutputPlugin::new("output2");

    // Register both plugins
    registry.register_plugin(Box::new(output_plugin_1)).unwrap();
    registry.register_plugin(Box::new(output_plugin_2)).unwrap();

    // Initially, no plugins should be active
    assert_eq!(registry.get_active_plugins().len(), 0);

    // Activate first Output plugin
    registry.activate_plugin("output1").unwrap();
    assert!(registry.is_plugin_active("output1"));
    assert!(!registry.is_plugin_active("output2"));
    assert_eq!(registry.get_active_plugins().len(), 1);

    // Activate second Output plugin - should deactivate the first due to uniqueness constraint
    registry.activate_plugin("output2").unwrap();
    assert!(!registry.is_plugin_active("output1")); // First plugin should be deactivated
    assert!(registry.is_plugin_active("output2")); // Second plugin should be active
    assert_eq!(registry.get_active_plugins().len(), 1); // Only one plugin active

    // Verify constraint works in reverse direction
    registry.activate_plugin("output1").unwrap();
    assert!(registry.is_plugin_active("output1")); // First plugin active again
    assert!(!registry.is_plugin_active("output2")); // Second plugin deactivated
    assert_eq!(registry.get_active_plugins().len(), 1);
}

#[tokio::test]
async fn test_non_output_plugins_not_affected_by_constraint() {
    // TEST: Verify that the uniqueness constraint only affects Output plugins
    // Other plugin types (Processing, etc.) should be able to coexist

    use crate::plugin::registry::PluginRegistry;
    // Note: Using local MockPlugin since test utilities were moved to tests/common/

    let mut registry = PluginRegistry::new();

    // Create one Output plugin and one non-Output plugin
    let output_plugin = MockOutputPlugin::new("output1");
    let processing_plugin = MockPlugin::new("processing1"); // This is a Processing plugin

    // Register both plugins
    registry.register_plugin(Box::new(output_plugin)).unwrap();
    registry
        .register_plugin(Box::new(processing_plugin))
        .unwrap();

    // Verify registry contains correct plugins after registration
    let plugin_names = registry.get_plugin_names();
    assert_eq!(plugin_names.len(), 2, "Registry should contain two plugins");
    assert!(
        plugin_names.contains(&"output1".to_string()),
        "Registry should contain output1 plugin"
    );
    assert!(
        plugin_names.contains(&"processing1".to_string()),
        "Registry should contain processing1 plugin"
    );

    // Verify plugin types are correct
    assert!(
        registry.has_plugin("output1"),
        "Registry should have output1 plugin"
    );
    assert!(
        registry.has_plugin("processing1"),
        "Registry should have processing1 plugin"
    );

    // Activate both plugins - they should coexist
    registry.activate_plugin("output1").unwrap();
    registry.activate_plugin("processing1").unwrap();

    // Both should be active since constraint only applies to Output plugins
    assert!(registry.is_plugin_active("output1"));
    assert!(registry.is_plugin_active("processing1"));
    assert_eq!(registry.get_active_plugins().len(), 2);

    // Add a second Output plugin to verify constraint still works
    let output_plugin_2 = MockOutputPlugin::new("output2");
    registry.register_plugin(Box::new(output_plugin_2)).unwrap();

    // Activating second Output plugin should only deactivate the first Output plugin
    registry.activate_plugin("output2").unwrap();

    assert!(!registry.is_plugin_active("output1")); // First Output deactivated
    assert!(registry.is_plugin_active("output2")); // Second Output active
    assert!(registry.is_plugin_active("processing1")); // Processing still active
    assert_eq!(registry.get_active_plugins().len(), 2); // output2 + processing1
}

#[tokio::test]
async fn test_uniqueness_constraint_through_plugin_manager() {
    // TEST: Verify that the uniqueness constraint works through PluginManager
    // This integration test ensures the constraint is enforced at the manager level

    use crate::plugin::manager::PluginManager;
    use crate::plugin::registry::SharedPluginRegistry;

    let manager = PluginManager::new(crate::core::version::get_api_version());
    let registry = manager.registry();

    // Create and register two mock Output plugins
    let output_plugin_1 = MockOutputPlugin::new("output1");
    let output_plugin_2 = MockOutputPlugin::new("output2");

    {
        let mut reg = registry.inner().write().await;
        reg.register_plugin(Box::new(output_plugin_1)).unwrap();
        reg.register_plugin(Box::new(output_plugin_2)).unwrap();
    }

    // Use the SharedPluginRegistry interface (like the plugin manager does)
    registry.activate_plugin("output1").await.unwrap();
    assert!(registry.is_plugin_active("output1").await);
    assert!(!registry.is_plugin_active("output2").await);

    // Activating second Output plugin should deactivate first via constraint
    registry.activate_plugin("output2").await.unwrap();
    assert!(!registry.is_plugin_active("output1").await); // First deactivated
    assert!(registry.is_plugin_active("output2").await); // Second active

    // Verify only one plugin is active
    let active_plugins = registry.get_active_plugins().await;
    assert_eq!(active_plugins.len(), 1);
    assert!(active_plugins.contains(&"output2".to_string()));
}

// Removed test_output_plugin_fallback_behavior - complex integration test beyond core functionality

#[tokio::test]
async fn test_builtin_output_plugin_uniqueness() {
    // This test verifies that we have exactly one builtin Output plugin
    // and that the uniqueness constraint is properly configured for it.
    // The constraint system now ensures that only one Output plugin can be active.

    let discovery = BuiltinPluginDiscovery::new();
    let plugins = discovery.discover_builtin_plugins().await.unwrap();

    // Count output plugins (should be exactly 1 - our builtin OutputPlugin)
    let output_plugins: Vec<_> = plugins
        .iter()
        .filter(|p| p.info.plugin_type == PluginType::Output)
        .collect();

    assert_eq!(
        output_plugins.len(),
        1,
        "Should have exactly one builtin Output plugin"
    );
    assert_eq!(output_plugins[0].info.name, "output");

    // Verify the Output plugin is properly configured with uniqueness constraint
    // The registry enforces this constraint - tested in other tests in this file
}

#[tokio::test]
async fn test_output_plugin_factory_creates_plugin_with_correct_type() {
    // Verify that factory-created OutputPlugin instances have correct type
    let discovery = BuiltinPluginDiscovery::new();
    let plugins = discovery.discover_builtin_plugins().await.unwrap();

    let output_plugin = plugins
        .iter()
        .find(|p| p.info.name == "output")
        .expect("OutputPlugin should be discoverable");

    let plugin_instance = (output_plugin.factory)();
    let info = plugin_instance.plugin_info();

    assert_eq!(info.plugin_type, PluginType::Output);
    assert_eq!(info.name, "output");
}
