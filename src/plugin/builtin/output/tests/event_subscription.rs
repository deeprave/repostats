//! Tests for OutputPlugin event subscription and handling

use crate::notifications::api::{
    AsyncNotificationManager, Event, PluginEvent, PluginEventType, ScanEvent, ScanEventType,
    SystemEvent, SystemEventType,
};
use crate::plugin::builtin::output::OutputPlugin;
use crate::plugin::data_export::PluginDataExport;
use crate::plugin::traits::Plugin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

#[tokio::test]
async fn test_plugin_can_subscribe_to_plugin_events() {
    let mut plugin = OutputPlugin::new();

    // Create and inject notification manager
    let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
    plugin.set_notification_manager(notification_manager.clone());

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test that plugin can create a plugin event subscription
    let result = plugin.subscribe_to_plugin_events().await;
    assert!(result.is_ok());

    let mut receiver = result.unwrap();

    // Publish a plugin event and verify we receive it
    let mut manager = notification_manager.lock().await;
    let test_event = Event::Plugin(PluginEvent {
        plugin_id: "test-plugin".to_string(),
        scan_id: "test-scan".to_string(),
        event_type: PluginEventType::Registered,
        timestamp: std::time::SystemTime::now(),
        message: Some("Test plugin registered".to_string()),
        data_export: None,
    });

    manager.publish(test_event.clone()).await.unwrap();
    drop(manager); // Release the lock

    // Verify we received the event
    let event_result = timeout(Duration::from_millis(100), receiver.recv()).await;
    assert!(event_result.is_ok());

    if let Ok(Some(received_event)) = event_result {
        match received_event {
            Event::Plugin(plugin_event) => {
                assert_eq!(plugin_event.plugin_id, "test-plugin");
                assert_eq!(plugin_event.event_type, PluginEventType::Registered);
            }
            _ => panic!("Expected PluginEvent"),
        }
    }
}

#[tokio::test]
async fn test_plugin_can_subscribe_to_scan_events() {
    let mut plugin = OutputPlugin::new();

    // Create and inject notification manager
    let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
    plugin.set_notification_manager(notification_manager.clone());

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test that plugin can create a scan event subscription
    let result = plugin.subscribe_to_scan_events().await;
    assert!(result.is_ok());

    let mut receiver = result.unwrap();

    // Publish a scan event and verify we receive it
    let mut manager = notification_manager.lock().await;
    let test_event = Event::Scan(ScanEvent {
        event_type: ScanEventType::Started,
        timestamp: std::time::SystemTime::now(),
        scan_id: "test-scan".to_string(),
        message: Some("Scan started".to_string()),
    });

    manager.publish(test_event.clone()).await.unwrap();
    drop(manager); // Release the lock

    // Verify we received the event
    let event_result = timeout(Duration::from_millis(100), receiver.recv()).await;
    assert!(event_result.is_ok());

    if let Ok(Some(received_event)) = event_result {
        match received_event {
            Event::Scan(scan_event) => {
                assert_eq!(scan_event.scan_id, "test-scan");
                assert_eq!(scan_event.event_type, ScanEventType::Started);
            }
            _ => panic!("Expected ScanEvent"),
        }
    }
}

#[tokio::test]
async fn test_plugin_can_subscribe_to_system_events() {
    let mut plugin = OutputPlugin::new();

    // Create and inject notification manager
    let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
    plugin.set_notification_manager(notification_manager.clone());

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test that plugin can create a system event subscription
    let result = plugin.subscribe_to_system_events().await;
    assert!(result.is_ok());

    let mut receiver = result.unwrap();

    // Publish a system event and verify we receive it
    let mut manager = notification_manager.lock().await;
    let test_event = Event::System(SystemEvent::new(SystemEventType::Shutdown));

    manager.publish(test_event.clone()).await.unwrap();
    drop(manager); // Release the lock

    // Verify we received the event
    let event_result = timeout(Duration::from_millis(100), receiver.recv()).await;
    assert!(event_result.is_ok());

    if let Ok(Some(received_event)) = event_result {
        match received_event {
            Event::System(system_event) => {
                assert_eq!(system_event.event_type, SystemEventType::Shutdown);
            }
            _ => panic!("Expected SystemEvent"),
        }
    }
}

#[tokio::test]
async fn test_plugin_event_subscription_requires_initialization() {
    let plugin = OutputPlugin::new();

    // Event subscriptions should fail if plugin is not initialized
    let result1 = plugin.subscribe_to_plugin_events().await;
    assert!(result1.is_err());

    let result2 = plugin.subscribe_to_scan_events().await;
    assert!(result2.is_err());

    let result3 = plugin.subscribe_to_system_events().await;
    assert!(result3.is_err());
}

#[tokio::test]
async fn test_plugin_can_handle_basic_events() {
    let mut plugin = OutputPlugin::new();

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test basic event handling functionality
    let result = plugin
        .handle_plugin_event(&PluginEvent {
            plugin_id: "test-plugin".to_string(),
            scan_id: "test-scan".to_string(),
            event_type: PluginEventType::Registered,
            timestamp: std::time::SystemTime::now(),
            message: Some("Test plugin registered".to_string()),
            data_export: None,
        })
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_plugin_handles_data_ready_events_with_payloads() {
    let mut plugin = OutputPlugin::new();

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Create mock data export
    let mut data_map = std::collections::HashMap::new();
    data_map.insert(
        "key".to_string(),
        crate::plugin::data_export::Value::String("value".to_string()),
    );
    let payload = crate::plugin::data_export::DataPayload::key_value(data_map);

    let data_export = Arc::new(PluginDataExport::new(
        "test-plugin".to_string(),
        "test-scan".to_string(),
        payload,
    ));

    // Test handling DataReady event with data payload
    let result = plugin
        .handle_plugin_event(&PluginEvent {
            plugin_id: "test-plugin".to_string(),
            scan_id: "test-scan".to_string(),
            event_type: PluginEventType::DataReady,
            timestamp: std::time::SystemTime::now(),
            message: Some("Data ready for export".to_string()),
            data_export: Some(data_export.clone()),
        })
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_plugin_can_subscribe_to_multiple_event_types_simultaneously() {
    let mut plugin = OutputPlugin::new();

    // Create and inject notification manager
    let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
    plugin.set_notification_manager(notification_manager.clone());

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test that plugin can create multiple concurrent subscriptions
    let plugin_receiver = plugin.subscribe_to_plugin_events().await;
    assert!(plugin_receiver.is_ok());

    let scan_receiver = plugin.subscribe_to_scan_events().await;
    assert!(scan_receiver.is_ok());

    let system_receiver = plugin.subscribe_to_system_events().await;
    assert!(system_receiver.is_ok());

    // All subscriptions should be active simultaneously
    let mut plugin_recv = plugin_receiver.unwrap();
    let mut scan_recv = scan_receiver.unwrap();
    let mut system_recv = system_receiver.unwrap();

    let mut manager = notification_manager.lock().await;

    // Publish events to each subscription
    let plugin_event = Event::Plugin(PluginEvent {
        plugin_id: "test-plugin".to_string(),
        scan_id: "test-scan".to_string(),
        event_type: PluginEventType::DataReady,
        timestamp: std::time::SystemTime::now(),
        message: Some("Plugin data ready".to_string()),
        data_export: None,
    });

    let scan_event = Event::Scan(ScanEvent {
        event_type: ScanEventType::Completed,
        timestamp: std::time::SystemTime::now(),
        scan_id: "test-scan".to_string(),
        message: Some("Scan completed".to_string()),
    });

    let system_event = Event::System(SystemEvent::new(SystemEventType::Shutdown));

    // Publish all events
    manager.publish(plugin_event).await.unwrap();
    manager.publish(scan_event).await.unwrap();
    manager.publish(system_event).await.unwrap();
    drop(manager); // Release the lock

    // Verify each subscription receives its respective event
    let plugin_result = timeout(Duration::from_millis(100), plugin_recv.recv()).await;
    assert!(plugin_result.is_ok());

    let scan_result = timeout(Duration::from_millis(100), scan_recv.recv()).await;
    assert!(scan_result.is_ok());

    let system_result = timeout(Duration::from_millis(100), system_recv.recv()).await;
    assert!(system_result.is_ok());
}

#[tokio::test]
async fn test_plugin_handles_error_scenarios_in_event_processing() {
    let mut plugin = OutputPlugin::new();

    // Create and inject notification manager
    let notification_manager = Arc::new(Mutex::new(AsyncNotificationManager::new()));
    plugin.set_notification_manager(notification_manager.clone());

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test handling of error events
    let scan_error_event = ScanEvent {
        event_type: ScanEventType::Error,
        timestamp: std::time::SystemTime::now(),
        scan_id: "test-scan".to_string(),
        message: Some("Scan error occurred".to_string()),
    };

    // For now, handle_plugin_event only handles PluginEvents,
    // but we test that plugin can at least receive scan error events
    let receiver = plugin.subscribe_to_scan_events().await.unwrap();
    let mut scan_recv = receiver;

    let mut manager = notification_manager.lock().await;
    let test_event = Event::Scan(scan_error_event);

    manager.publish(test_event).await.unwrap();
    drop(manager); // Release the lock

    // Verify we can receive error events
    let event_result = timeout(Duration::from_millis(100), scan_recv.recv()).await;
    assert!(event_result.is_ok());

    if let Ok(Some(received_event)) = event_result {
        match received_event {
            Event::Scan(scan_event) => {
                assert_eq!(scan_event.event_type, ScanEventType::Error);
            }
            _ => panic!("Expected ScanEvent"),
        }
    }
}

#[tokio::test]
async fn test_plugin_handles_scan_events() {
    let mut plugin = OutputPlugin::new();

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test handling scan completion event
    let scan_completed_event = crate::notifications::api::ScanEvent {
        event_type: ScanEventType::Completed,
        timestamp: std::time::SystemTime::now(),
        scan_id: "test-scan".to_string(),
        message: Some("Scan completed successfully".to_string()),
    };

    let result = plugin.handle_scan_event(&scan_completed_event).await;
    assert!(result.is_ok());

    // Test handling scan error event
    let scan_error_event = crate::notifications::api::ScanEvent {
        event_type: ScanEventType::Error,
        timestamp: std::time::SystemTime::now(),
        scan_id: "test-scan".to_string(),
        message: Some("Scan error occurred".to_string()),
    };

    let result = plugin.handle_scan_event(&scan_error_event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_plugin_handles_system_events() {
    let mut plugin = OutputPlugin::new();

    // Initialize plugin
    plugin.initialize().await.unwrap();

    // Test handling system shutdown event
    let shutdown_event = crate::notifications::api::SystemEvent::new(SystemEventType::Shutdown);

    let result = plugin.handle_system_event(&shutdown_event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_event_handlers_require_initialization() {
    let mut plugin = OutputPlugin::new();

    // All event handlers should fail if plugin is not initialized
    let plugin_event = PluginEvent {
        plugin_id: "test".to_string(),
        scan_id: "test".to_string(),
        event_type: PluginEventType::DataReady,
        timestamp: std::time::SystemTime::now(),
        message: None,
        data_export: None,
    };

    let scan_event = crate::notifications::api::ScanEvent {
        event_type: ScanEventType::Completed,
        timestamp: std::time::SystemTime::now(),
        scan_id: "test".to_string(),
        message: None,
    };

    let system_event = crate::notifications::api::SystemEvent::new(SystemEventType::Shutdown);

    assert!(plugin.handle_plugin_event(&plugin_event).await.is_err());
    assert!(plugin.handle_scan_event(&scan_event).await.is_err());
    assert!(plugin.handle_system_event(&system_event).await.is_err());
}

#[tokio::test]
async fn test_scan_started_event_stores_repository_context() {
    let mut plugin = OutputPlugin::new();
    plugin.initialize().await.unwrap();

    // Test handling ScanStarted event
    let scan_started_event = crate::notifications::api::ScanEvent {
        event_type: ScanEventType::Started,
        timestamp: std::time::SystemTime::now(),
        scan_id: "test-scan-123".to_string(),
        message: Some("/path/to/repository".to_string()),
    };

    let result = plugin.handle_scan_event(&scan_started_event).await;
    assert!(result.is_ok());

    // Verify repository context was stored
    assert!(plugin.repository_contexts.contains_key("test-scan-123"));
    let context = plugin.repository_contexts.get("test-scan-123").unwrap();
    assert_eq!(context.repository_path, "/path/to/repository");
    assert_eq!(context.scan_id, "test-scan-123");
    assert!(context.git_ref.is_none());
}

#[tokio::test]
async fn test_data_ready_event_triggers_immediate_export() {
    let mut plugin = OutputPlugin::new();
    plugin.initialize().await.unwrap();

    // First store repository context
    let scan_started_event = crate::notifications::api::ScanEvent {
        event_type: ScanEventType::Started,
        timestamp: std::time::SystemTime::now(),
        scan_id: "test-scan-456".to_string(),
        message: Some("/path/to/repo".to_string()),
    };
    plugin.handle_scan_event(&scan_started_event).await.unwrap();

    // Create mock data export
    let mut data_map = std::collections::HashMap::new();
    data_map.insert(
        "test_key".to_string(),
        crate::plugin::data_export::Value::String("test_value".to_string()),
    );
    let payload = crate::plugin::data_export::DataPayload::key_value(data_map);

    let data_export = Arc::new(PluginDataExport::new(
        "test-processor".to_string(),
        "test-scan-456".to_string(),
        payload,
    ));

    // Test DataReady event processing
    let data_ready_event = PluginEvent {
        plugin_id: "test-processor".to_string(),
        scan_id: "test-scan-456".to_string(),
        event_type: PluginEventType::DataReady,
        timestamp: std::time::SystemTime::now(),
        message: Some("Data ready for export".to_string()),
        data_export: Some(data_export.clone()),
    };

    let result = plugin.handle_plugin_event(&data_ready_event).await;
    assert!(result.is_ok());

    // Verify data was stored
    let key = ("test-processor".to_string(), "test-scan-456".to_string());
    assert!(plugin.received_data.contains_key(&key));
}

#[tokio::test]
async fn test_multiple_plugins_independent_processing() {
    let mut plugin = OutputPlugin::new();
    plugin.initialize().await.unwrap();

    // Store repository context
    let scan_started_event = crate::notifications::api::ScanEvent {
        event_type: ScanEventType::Started,
        timestamp: std::time::SystemTime::now(),
        scan_id: "multi-scan".to_string(),
        message: Some("/path/to/multi-repo".to_string()),
    };
    plugin.handle_scan_event(&scan_started_event).await.unwrap();

    // Create data from plugin A
    let mut data_a = std::collections::HashMap::new();
    data_a.insert(
        "plugin_a_data".to_string(),
        crate::plugin::data_export::Value::String("data_a_value".to_string()),
    );
    let payload_a = crate::plugin::data_export::DataPayload::key_value(data_a);
    let export_a = Arc::new(PluginDataExport::new("plugin-a", "multi-scan", payload_a));

    // Create data from plugin B
    let mut data_b = std::collections::HashMap::new();
    data_b.insert(
        "plugin_b_data".to_string(),
        crate::plugin::data_export::Value::String("data_b_value".to_string()),
    );
    let payload_b = crate::plugin::data_export::DataPayload::key_value(data_b);
    let export_b = Arc::new(PluginDataExport::new("plugin-b", "multi-scan", payload_b));

    // Process DataReady from plugin A
    let event_a = PluginEvent {
        plugin_id: "plugin-a".to_string(),
        scan_id: "multi-scan".to_string(),
        event_type: PluginEventType::DataReady,
        timestamp: std::time::SystemTime::now(),
        message: None,
        data_export: Some(export_a),
    };

    // Process DataReady from plugin B
    let event_b = PluginEvent {
        plugin_id: "plugin-b".to_string(),
        scan_id: "multi-scan".to_string(),
        event_type: PluginEventType::DataReady,
        timestamp: std::time::SystemTime::now(),
        message: None,
        data_export: Some(export_b),
    };

    // Both should process independently and successfully
    assert!(plugin.handle_plugin_event(&event_a).await.is_ok());
    assert!(plugin.handle_plugin_event(&event_b).await.is_ok());

    // Verify both data sets were stored independently
    let key_a = ("plugin-a".to_string(), "multi-scan".to_string());
    let key_b = ("plugin-b".to_string(), "multi-scan".to_string());
    assert!(plugin.received_data.contains_key(&key_a));
    assert!(plugin.received_data.contains_key(&key_b));
}
