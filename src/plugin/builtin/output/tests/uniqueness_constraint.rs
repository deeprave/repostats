//! Tests demonstrating the uniqueness constraint API gap for Output plugins
//!
//! These tests identify the current limitation where multiple Output plugins
//! could be active simultaneously, violating the "only one Output plugin active" constraint.

use crate::plugin::types::{PluginSource, PluginType};
use crate::plugin::unified_discovery::BuiltinPluginDiscovery;

#[tokio::test]
async fn test_output_plugin_uniqueness_constraint_gap() {
    // DEMONSTRATION OF API GAP: Current plugin system has no mechanism to enforce
    // that only one Output plugin can be active at a time.
    //
    // This test shows the problem: both builtin OutputPlugin and a hypothetical
    // external OutputPlugin could be activated simultaneously, which should not be allowed.

    let discovery = BuiltinPluginDiscovery::new();
    let plugins = discovery.discover_builtin_plugins().await.unwrap();

    // Find the output plugin
    let output_plugin = plugins
        .iter()
        .find(|p| p.info.plugin_type == PluginType::Output)
        .expect("OutputPlugin should be discoverable");

    // CURRENT PROBLEM: The system has no API to:
    // 1. Check if activating this Output plugin would conflict with another Output plugin
    // 2. Automatically deactivate any existing Output plugin when this one is activated
    // 3. Provide feedback about what plugins were deactivated due to conflicts

    // The current PluginRegistry::activate_plugin() method simply adds to a HashSet
    // without any constraint checking based on plugin type.

    // WHAT WE NEED:
    // - Method like: registry.activate_plugin_with_constraints(plugin_name, policy)
    // - Where policy could be: ExclusiveByType(PluginType::Output)
    // - Return type: Result<ActivationResult> where ActivationResult includes:
    //   - activated: String (the plugin that was activated)
    //   - deactivated: Vec<String> (plugins that were deactivated due to conflicts)

    assert_eq!(output_plugin.info.name, "output");
    assert_eq!(output_plugin.info.plugin_type, PluginType::Output);

    // TODO: Once uniqueness constraint API is implemented, this test should verify:
    // 1. Two Output plugins cannot be active simultaneously
    // 2. Activating a second Output plugin deactivates the first
    // 3. The system provides clear feedback about constraint enforcement
}

#[tokio::test]
async fn test_multiple_output_plugins_would_violate_constraints() {
    // This test documents what SHOULD happen once the API is implemented:
    // Only one Output plugin should be allowed to be active at any given time.

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

    // FUTURE STATE: If we had multiple Output plugins discovered,
    // the constraint system should enforce that only one is active.
    // This might happen when external Output plugins are supported.
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

    if let PluginSource::Builtin { factory } = &output_plugin.source {
        let plugin_instance = factory();
        let info = plugin_instance.plugin_info();

        assert_eq!(info.plugin_type, PluginType::Output);
        assert_eq!(info.name, "output");
    } else {
        panic!("OutputPlugin should have Builtin source type");
    }
}
