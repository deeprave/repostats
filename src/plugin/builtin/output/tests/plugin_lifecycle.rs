//! Tests for OutputPlugin lifecycle management (initialize, execute, cleanup)

use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::args::PluginConfig;
use crate::plugin::builtin::output::OutputPlugin;
use crate::plugin::traits::Plugin;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_plugin_initialization() {
    let mut plugin = OutputPlugin::new();

    // Plugin should not be initialized initially
    assert!(!plugin.initialized);

    // Initialize should succeed
    let result = plugin.initialize().await;
    assert!(result.is_ok());
    assert!(plugin.initialized);
}

#[tokio::test]
async fn test_plugin_double_initialization_fails() {
    let mut plugin = OutputPlugin::new();

    // First initialization should succeed
    let result1 = plugin.initialize().await;
    assert!(result1.is_ok());
    assert!(plugin.initialized);

    // Second initialization should fail
    let result2 = plugin.initialize().await;
    assert!(result2.is_err());

    let error_msg = result2.unwrap_err().to_string();
    assert!(error_msg.contains("already initialized"));
}

#[tokio::test]
async fn test_plugin_execute_requires_initialization() {
    let mut plugin = OutputPlugin::new();

    // Execute should fail when not initialized
    let result = plugin.execute(&[]).await;
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("not initialized"));
}

#[tokio::test]
async fn test_plugin_execute_after_initialization() {
    let mut plugin = OutputPlugin::new();

    // Create and inject notification manager
    let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
    plugin.set_notification_manager(notification_manager.clone());

    // Initialize first
    plugin.initialize().await.unwrap();

    // Execute should succeed after initialization
    let result = plugin.execute(&[]).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_plugin_cleanup() {
    let mut plugin = OutputPlugin::new();

    // Initialize plugin
    plugin.initialize().await.unwrap();
    assert!(plugin.initialized);

    // Cleanup should succeed and reset initialization state
    let result = plugin.cleanup().await;
    assert!(result.is_ok());
    assert!(!plugin.initialized);
}

#[tokio::test]
async fn test_plugin_argument_parsing() {
    let mut plugin = OutputPlugin::new();
    let config = PluginConfig::default();

    // Argument parsing should succeed (basic implementation)
    let result = plugin
        .parse_plugin_arguments(&["output".to_string()], &config)
        .await;
    assert!(result.is_ok());

    // Should also work with some arguments
    let result = plugin
        .parse_plugin_arguments(
            &[
                "output".to_string(),
                "--outfile".to_string(),
                "test.json".to_string(),
            ],
            &config,
        )
        .await;
    assert!(result.is_ok());
}
