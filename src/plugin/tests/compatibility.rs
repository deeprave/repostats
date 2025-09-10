//! Focused tests for plugin compatibility system
//!
//! These tests verify that:
//! - The default is_compatible() implementation returns false
//! - All builtin plugins properly override is_compatible()
//! - Compatibility checks work correctly for different API versions

use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::args::PluginConfig;
use crate::plugin::error::PluginResult;
use crate::plugin::traits::Plugin;
use crate::plugin::types::{PluginFunction, PluginInfo, PluginType};
use crate::scanner::types::ScanRequires;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Test plugin that uses the default is_compatible implementation
struct DefaultCompatibilityPlugin;

#[async_trait::async_trait]
impl Plugin for DefaultCompatibilityPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: "default-compat".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin with default compatibility".to_string(),
            author: "Test".to_string(),
            api_version: 1,
            plugin_type: PluginType::Processing,
            functions: vec![],
            required: ScanRequires::NONE,
            auto_active: false,
        }
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Processing
    }

    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![]
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

    // Intentionally NOT overriding is_compatible to test default behavior
}

/// Test plugin with custom compatibility logic
struct CustomCompatibilityPlugin {
    min_version: u32,
}

#[async_trait::async_trait]
impl Plugin for CustomCompatibilityPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: "custom-compat".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin with custom compatibility".to_string(),
            author: "Test".to_string(),
            api_version: self.min_version,
            plugin_type: PluginType::Processing,
            functions: vec![],
            required: ScanRequires::NONE,
            auto_active: false,
        }
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Processing
    }

    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![]
    }

    fn is_compatible(&self, system_api_version: u32) -> bool {
        // Custom logic: system version must be at least min_version
        system_api_version >= self.min_version
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

#[test]
fn test_default_is_compatible_returns_false() {
    // Test that the default implementation returns false
    let plugin = DefaultCompatibilityPlugin;

    // Should return false for any version
    assert!(!plugin.is_compatible(0));
    assert!(!plugin.is_compatible(1));
    assert!(!plugin.is_compatible(100));
    assert!(!plugin.is_compatible(u32::MAX));
}

#[test]
fn test_custom_compatibility_logic() {
    // Test custom compatibility implementation
    let plugin = CustomCompatibilityPlugin { min_version: 100 };

    // Should return false for versions below min_version
    assert!(!plugin.is_compatible(0));
    assert!(!plugin.is_compatible(50));
    assert!(!plugin.is_compatible(99));

    // Should return true for versions at or above min_version
    assert!(plugin.is_compatible(100));
    assert!(plugin.is_compatible(101));
    assert!(plugin.is_compatible(1000));
}

#[test]
fn test_builtin_dump_plugin_compatibility() {
    // Test that DumpPlugin properly implements is_compatible
    let plugin = crate::plugin::builtin::dump::DumpPlugin::new();
    let current_version = crate::core::version::get_api_version();

    // Should be compatible with current version
    assert!(plugin.is_compatible(current_version));

    // Should be compatible with higher versions
    assert!(plugin.is_compatible(current_version + 1));
    assert!(plugin.is_compatible(current_version + 100));

    // Should not be compatible with lower versions
    if current_version > 0 {
        assert!(!plugin.is_compatible(current_version - 1));
    }
    assert!(!plugin.is_compatible(0));
}

#[test]
fn test_builtin_output_plugin_compatibility() {
    // Test that OutputPlugin properly implements is_compatible
    let plugin = crate::plugin::builtin::output::OutputPlugin::new();
    let current_version = crate::core::version::get_api_version();

    // Should be compatible with current version
    assert!(plugin.is_compatible(current_version));

    // Should be compatible with higher versions
    assert!(plugin.is_compatible(current_version + 1));
    assert!(plugin.is_compatible(current_version + 100));

    // Should not be compatible with lower versions
    if current_version > 0 {
        assert!(!plugin.is_compatible(current_version - 1));
    }
    assert!(!plugin.is_compatible(0));
}

#[tokio::test]
async fn test_all_builtin_plugins_override_is_compatible() {
    // Verify that ALL builtin plugins override is_compatible
    // This test will fail if a new builtin plugin forgets to implement is_compatible

    use crate::plugin::unified_discovery::PluginDiscovery;

    let discovery = PluginDiscovery::with_inclusion_config(None, vec![], true, false);
    let plugins = discovery
        .discover_plugins()
        .await
        .expect("Failed to discover plugins");

    assert!(!plugins.is_empty(), "Should have builtin plugins");

    let current_version = crate::core::version::get_api_version();

    for discovered in plugins {
        let plugin = match discovered.source {
            crate::plugin::types::PluginSource::Builtin { factory } => factory(),
            crate::plugin::types::PluginSource::BuiltinConsumer { factory } => {
                let mut consumer_plugin = factory();
                // ConsumerPlugin extends Plugin, so we can get a reference to the Plugin trait
                let plugin_ref = consumer_plugin.as_mut() as &mut dyn crate::plugin::traits::Plugin;
                // Check compatibility directly on the consumer plugin
                assert!(
                    consumer_plugin.is_compatible(current_version),
                    "Plugin '{}' should be compatible with current API version",
                    discovered.info.name
                );
                continue; // Skip the general check below since we already checked
            }
            _ => continue,
        };

        // All builtin plugins should be compatible with current version
        assert!(
            plugin.is_compatible(current_version),
            "Plugin '{}' should be compatible with current API version",
            discovered.info.name
        );

        // All builtin plugins should be compatible with higher versions
        assert!(
            plugin.is_compatible(current_version + 100),
            "Plugin '{}' should be compatible with future API versions",
            discovered.info.name
        );

        // All builtin plugins should NOT be compatible with older versions
        if current_version > 0 {
            assert!(
                !plugin.is_compatible(current_version - 1),
                "Plugin '{}' should not be compatible with older API versions",
                discovered.info.name
            );
        }
    }
}

#[test]
fn test_compatibility_edge_cases() {
    let plugin = CustomCompatibilityPlugin { min_version: 100 };

    // Test boundary conditions
    assert!(!plugin.is_compatible(99));
    assert!(plugin.is_compatible(100));
    assert!(plugin.is_compatible(101));

    // Test extreme values
    assert!(!plugin.is_compatible(0));
    assert!(plugin.is_compatible(u32::MAX));
}

#[test]
fn test_external_plugin_stub_compatibility() {
    // This test documents expected behavior for future external plugins
    // External plugins should set their min version based on compilation environment

    struct ExternalPluginStub {
        compiled_against_version: u32,
    }

    impl ExternalPluginStub {
        fn is_compatible(&self, system_api_version: u32) -> bool {
            // External plugins typically require at least the version they were compiled against
            system_api_version >= self.compiled_against_version
        }
    }

    let external = ExternalPluginStub {
        compiled_against_version: 20250101,
    };

    assert!(!external.is_compatible(20240101));
    assert!(external.is_compatible(20250101));
    assert!(external.is_compatible(20260101));
}
