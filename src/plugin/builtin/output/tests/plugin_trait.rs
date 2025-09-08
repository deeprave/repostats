//! Tests for OutputPlugin's Plugin trait implementation

use crate::plugin::builtin::output::OutputPlugin;
use crate::plugin::traits::Plugin;

#[tokio::test]
async fn test_output_plugin_exists_and_implements_plugin_trait() {
    let plugin = OutputPlugin::new();

    // Verify the plugin exists and implements Plugin trait
    let _: &dyn Plugin = &plugin;
}

#[tokio::test]
async fn test_output_plugin_creation() {
    let plugin = OutputPlugin::new();

    // Verify plugin is created in uninitialized state
    assert!(!plugin.initialized);
}

#[tokio::test]
async fn test_output_plugin_default() {
    let plugin = OutputPlugin::default();

    // Verify default creation works the same as new()
    assert!(!plugin.initialized);
}
