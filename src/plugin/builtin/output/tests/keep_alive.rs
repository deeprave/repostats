//! Tests for OutputPlugin keep-alive mechanism

use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::builtin::output::OutputPlugin;
use crate::plugin::traits::Plugin;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_plugin_sends_keepalive_during_long_operation() {
    let mut plugin = OutputPlugin::new();

    // Inject notification manager
    let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
    plugin.set_notification_manager(notification_manager);

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test that the keep-alive method works without errors
    let result = plugin
        .send_keepalive_signal("Processing large dataset")
        .await;
    assert!(result.is_ok());

    // Test with different messages
    let result2 = plugin.send_keepalive_signal("Still working...").await;
    assert!(result2.is_ok());
}

#[tokio::test]
async fn test_plugin_keepalive_multiple_signals() {
    let mut plugin = OutputPlugin::new();

    // Inject notification manager
    let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
    plugin.set_notification_manager(notification_manager);

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test that keep-alive signals can be sent multiple times
    let result1 = plugin.send_keepalive_signal("Step 1").await;
    assert!(result1.is_ok());

    // Small delay between signals
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let result2 = plugin.send_keepalive_signal("Step 2").await;
    assert!(result2.is_ok());
}

#[tokio::test]
async fn test_plugin_keepalive_requires_initialization() {
    let plugin = OutputPlugin::new();

    // Keep-alive should fail if plugin is not initialized (no notification manager injected)
    let result = plugin.send_keepalive_signal("Should fail").await;
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    // Now it should fail because no notification manager is set, not because it's not initialized
    assert!(error_msg.contains("not initialized"));
}
