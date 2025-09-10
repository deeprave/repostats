//! Tests for OutputPlugin basic structure and setup

use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::builtin::output::OutputPlugin;
use crate::plugin::traits::Plugin;
use crate::plugin::types::PluginType;
use crate::scanner::types::ScanRequires;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_output_plugin_creation() {
    let plugin = OutputPlugin::new();

    // Verify basic plugin properties
    let info = plugin.plugin_info();
    assert_eq!(info.name, "output");
    assert_eq!(plugin.plugin_type(), PluginType::Output);
    assert_eq!(info.version, "1.0.0");
    assert_eq!(plugin.requirements(), ScanRequires::NONE);
}

#[tokio::test]
async fn test_output_plugin_identification() {
    let plugin = OutputPlugin::new();

    let info = plugin.plugin_info();
    assert_eq!(info.name, "output");
    assert_eq!(info.plugin_type, PluginType::Output);
    assert_eq!(info.version, "1.0.0");
    assert!(!info.description.is_empty());
    assert!(!info.author.is_empty());
}

#[tokio::test]
async fn test_output_plugin_lifecycle() {
    let mut plugin = OutputPlugin::new();

    // Create and inject notification manager
    let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
    plugin.set_notification_manager(notification_manager.clone());

    // Test initialization
    let init_result = plugin.initialize().await;
    assert!(init_result.is_ok());

    // Test execute
    let exec_result = plugin.execute(&[]).await;
    assert!(exec_result.is_ok());

    // Test cleanup
    let cleanup_result = plugin.cleanup().await;
    assert!(cleanup_result.is_ok());
}

#[tokio::test]
async fn test_output_plugin_no_scan_requirements() {
    let plugin = OutputPlugin::new();

    // OutputPlugin should not require scan data (doesn't consume from queues)
    assert_eq!(plugin.requirements(), ScanRequires::NONE);

    // Verify it advertises output functions
    let functions = plugin.advertised_functions();
    assert!(!functions.is_empty()); // Should have output function
    assert_eq!(functions[0].name, "output");
}

#[tokio::test]
async fn test_output_plugin_configuration() {
    let mut plugin = OutputPlugin::new();

    // Test that plugin can be initialized successfully
    assert!(plugin.initialize().await.is_ok());

    // Test that second initialization returns error (plugin already initialized)
    assert!(plugin.initialize().await.is_err());

    // Test cleanup and re-initialization
    assert!(plugin.cleanup().await.is_ok());
    assert!(plugin.initialize().await.is_ok());
}

#[test]
fn test_output_plugin_type_identification() {
    let plugin = OutputPlugin::new();

    // Verify it identifies as Output type
    assert_eq!(plugin.plugin_type(), PluginType::Output);

    // Should not be any other type
    assert_ne!(plugin.plugin_type(), PluginType::Processing);
    assert_ne!(plugin.plugin_type(), PluginType::Notification);
}

#[tokio::test]
async fn test_output_plugin_settings_integration() {
    let mut plugin = OutputPlugin::new();

    // Test that plugin works without requiring specific settings
    // (Settings integration will be added in later phases)
    assert!(plugin.initialize().await.is_ok());
}

#[tokio::test]
async fn test_output_plugin_uniqueness_ready() {
    // Test that OutputPlugin is ready for uniqueness constraints
    // (The uniqueness logic will be in plugin manager)
    let plugin1 = OutputPlugin::new();
    let plugin2 = OutputPlugin::new();

    assert_eq!(plugin1.plugin_type(), plugin2.plugin_type());
    assert_eq!(plugin1.plugin_info().name, plugin2.plugin_info().name);
}
