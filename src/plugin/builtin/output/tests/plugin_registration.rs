//! Tests for OutputPlugin registration and discovery

use crate::plugin::discovery::BuiltinPluginDiscovery;
use crate::plugin::types::PluginType;

#[tokio::test]
async fn test_output_plugin_appears_in_builtin_discovery() {
    let discovery = BuiltinPluginDiscovery::new();
    let plugins = discovery.discover_builtin_plugins().await.unwrap();

    // Should have both dump and output plugins now
    assert_eq!(plugins.len(), 2);

    // Find the output plugin
    let output_plugin = plugins
        .iter()
        .find(|p| p.info.name == "output")
        .expect("OutputPlugin should be discoverable");

    assert_eq!(output_plugin.info.plugin_type, PluginType::Output);
    assert_eq!(output_plugin.info.version, "1.0.0");
    assert_eq!(output_plugin.info.author, "repostats built-in");
}

#[tokio::test]
async fn test_output_plugin_factory_creates_working_instance() {
    let discovery = BuiltinPluginDiscovery::new();
    let plugins = discovery.discover_builtin_plugins().await.unwrap();

    let output_plugin = plugins
        .iter()
        .find(|p| p.info.name == "output")
        .expect("OutputPlugin should be discoverable");

    // Test that the factory can create a plugin instance
    let plugin_instance = (output_plugin.factory)();

    // Verify it's the correct type by checking plugin info
    let info = plugin_instance.plugin_info();
    assert_eq!(info.name, "output");
    assert_eq!(info.plugin_type, PluginType::Output);
}
