//! Tests for event subscription and handling framework

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::notifications::api::{
    get_notification_service, AsyncNotificationManager, Event, EventFilter, PluginEvent,
    PluginEventType,
};
use crate::plugin::data_export::{DataPayload, PluginDataExport};

#[tokio::test]
async fn test_event_subscription_filtering() {
    // Test that EventFilter::PluginOnly correctly filters plugin events
    let mut notification_manager = AsyncNotificationManager::new();

    // Subscribe to plugin events only
    let receiver_result = notification_manager.subscribe(
        "test-subscriber-1".to_string(),
        EventFilter::PluginOnly,
        "test-source".to_string(),
    );
    assert!(receiver_result.is_ok());

    let mut receiver = receiver_result.unwrap();

    // Publish a plugin event
    let plugin_event = Event::Plugin(PluginEvent::new(
        PluginEventType::Processing,
        "test-plugin".to_string(),
        "test-scan".to_string(),
    ));

    notification_manager
        .publish(plugin_event.clone())
        .await
        .unwrap();

    // Verify we receive the plugin event
    let event_result = timeout(Duration::from_millis(100), receiver.recv()).await;
    assert!(event_result.is_ok());
    assert!(event_result.unwrap().is_some());

    // Publish a non-plugin event (should not be received)
    let system_event = Event::System(crate::notifications::api::SystemEvent::new(
        crate::notifications::api::SystemEventType::Startup,
    ));

    notification_manager.publish(system_event).await.unwrap();

    // Verify we don't receive the system event (timeout)
    let timeout_result = timeout(Duration::from_millis(50), receiver.recv()).await;
    assert!(timeout_result.is_err());
}

#[tokio::test]
async fn test_data_ready_event_handling() {
    // Test handling of DataReady events with data export payload
    let mut notification_manager = AsyncNotificationManager::new();

    let receiver_result = notification_manager.subscribe(
        "test-subscriber-data-ready".to_string(),
        EventFilter::PluginOnly,
        "test-source".to_string(),
    );
    assert!(receiver_result.is_ok());

    let mut receiver = receiver_result.unwrap();

    // Create data export payload
    let plugin_id = "test-plugin".to_string();
    let scan_id = "test-scan-123".to_string();

    let data_export = Arc::new(
        PluginDataExport::builder(plugin_id.clone(), scan_id.clone())
            .payload(DataPayload::raw(
                "test result data".to_string(),
                Some("text/plain".to_string()),
            ))
            .build()
            .unwrap(),
    );

    // Create and publish DataReady event
    let data_ready_event = Event::Plugin(PluginEvent::with_data_export(
        PluginEventType::DataReady,
        plugin_id.clone(),
        scan_id.clone(),
        data_export.clone(),
    ));

    notification_manager
        .publish(data_ready_event)
        .await
        .unwrap();

    // Verify we receive the event and can access the data
    let event_result = timeout(Duration::from_millis(100), receiver.recv()).await;
    assert!(event_result.is_ok());

    let received_event = event_result.unwrap();
    assert!(received_event.is_some());

    match received_event.unwrap() {
        Event::Plugin(plugin_event) => {
            assert_eq!(plugin_event.event_type, PluginEventType::DataReady);
            assert_eq!(plugin_event.plugin_id, plugin_id);
            assert_eq!(plugin_event.scan_id, scan_id);
            assert!(plugin_event.data_export.is_some());

            let data = plugin_event.data_export.unwrap();
            assert_eq!(data.plugin_id, plugin_id);
            assert_eq!(data.scan_id, scan_id);
        }
        _ => panic!("Expected Plugin event"),
    }
}

#[tokio::test]
async fn test_multiple_subscribers_same_event() {
    // Test that multiple subscribers can receive the same event
    let mut notification_manager = AsyncNotificationManager::new();

    // Create two subscribers
    let receiver1_result = notification_manager.subscribe(
        "test-subscriber-multi-1".to_string(),
        EventFilter::PluginOnly,
        "test-source".to_string(),
    );
    let receiver2_result = notification_manager.subscribe(
        "test-subscriber-multi-2".to_string(),
        EventFilter::PluginOnly,
        "test-source".to_string(),
    );

    assert!(receiver1_result.is_ok());
    assert!(receiver2_result.is_ok());

    let mut receiver1 = receiver1_result.unwrap();
    let mut receiver2 = receiver2_result.unwrap();

    // Publish a plugin event
    let plugin_event = Event::Plugin(PluginEvent::with_message(
        PluginEventType::Completed,
        "test-plugin".to_string(),
        "test-scan".to_string(),
        "Plugin completed successfully".to_string(),
    ));

    notification_manager.publish(plugin_event).await.unwrap();

    // Both subscribers should receive the event
    let event1_result = timeout(Duration::from_millis(100), receiver1.recv()).await;
    let event2_result = timeout(Duration::from_millis(100), receiver2.recv()).await;

    assert!(event1_result.is_ok());
    assert!(event2_result.is_ok());

    assert!(event1_result.unwrap().is_some());
    assert!(event2_result.unwrap().is_some());
}

#[tokio::test]
async fn test_event_filter_combinations() {
    // Test different event filter combinations work correctly
    let mut notification_manager = AsyncNotificationManager::new();

    // Subscribe to scan and plugin events
    let receiver_result = notification_manager.subscribe(
        "test-subscriber-filter-combo".to_string(),
        EventFilter::ScanAndPlugin,
        "test-source".to_string(),
    );
    assert!(receiver_result.is_ok());
    let mut receiver = receiver_result.unwrap();

    // Publish plugin event (should be received)
    let plugin_event = Event::Plugin(PluginEvent::new(
        PluginEventType::Registered,
        "test-plugin".to_string(),
        "test-scan".to_string(),
    ));

    notification_manager.publish(plugin_event).await.unwrap();

    let event_result = timeout(Duration::from_millis(100), receiver.recv()).await;
    assert!(event_result.is_ok());
    assert!(event_result.unwrap().is_some());

    // Publish scan event (should be received)
    let scan_event = Event::Scan(crate::notifications::api::ScanEvent::new(
        crate::notifications::api::ScanEventType::Started,
        "test-scan".to_string(),
    ));

    notification_manager.publish(scan_event).await.unwrap();

    let scan_event_result = timeout(Duration::from_millis(100), receiver.recv()).await;
    assert!(scan_event_result.is_ok());
    assert!(scan_event_result.unwrap().is_some());

    // Publish system event (should not be received)
    let system_event = Event::System(crate::notifications::api::SystemEvent::new(
        crate::notifications::api::SystemEventType::Startup,
    ));

    notification_manager.publish(system_event).await.unwrap();

    // Should timeout (not receive system event)
    let timeout_result = timeout(Duration::from_millis(50), receiver.recv()).await;
    assert!(timeout_result.is_err());
}

#[tokio::test]
async fn test_event_handler_with_message_and_data() {
    // Test handling events that have both message and data export
    let mut notification_manager = AsyncNotificationManager::new();

    let receiver_result = notification_manager.subscribe(
        "test-subscriber-complex".to_string(),
        EventFilter::PluginOnly,
        "test-source".to_string(),
    );
    assert!(receiver_result.is_ok());
    let mut receiver = receiver_result.unwrap();

    // Create data export
    let plugin_id = "advanced-plugin".to_string();
    let scan_id = "advanced-scan-456".to_string();
    let message = "Advanced processing complete with results".to_string();

    let data_export = Arc::new(
        PluginDataExport::builder(plugin_id.clone(), scan_id.clone())
            .payload(DataPayload::key_value({
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "status".to_string(),
                    crate::plugin::data_export::Value::String("success".to_string()),
                );
                map.insert(
                    "processed_items".to_string(),
                    crate::plugin::data_export::Value::Integer(42),
                );
                map
            }))
            .build()
            .unwrap(),
    );

    // Create event with both message and data
    let advanced_event = Event::Plugin(PluginEvent::with_data_export_and_message(
        PluginEventType::DataReady,
        plugin_id.clone(),
        scan_id.clone(),
        data_export.clone(),
        message.clone(),
    ));

    notification_manager.publish(advanced_event).await.unwrap();

    // Verify we receive and can handle the complex event
    let event_result = timeout(Duration::from_millis(100), receiver.recv()).await;
    assert!(event_result.is_ok());

    let received_event = event_result.unwrap();
    assert!(received_event.is_some());

    match received_event.unwrap() {
        Event::Plugin(plugin_event) => {
            assert_eq!(plugin_event.event_type, PluginEventType::DataReady);
            assert_eq!(plugin_event.plugin_id, plugin_id);
            assert_eq!(plugin_event.scan_id, scan_id);
            assert_eq!(plugin_event.message, Some(message));
            assert!(plugin_event.data_export.is_some());

            // Verify the data export content
            let data = plugin_event.data_export.unwrap();
            assert_eq!(data.plugin_id, plugin_id);
            assert_eq!(data.scan_id, scan_id);

            // Verify key-value payload
            match &data.payload {
                crate::plugin::data_export::DataPayload::KeyValue { data: kv_data } => {
                    assert!(kv_data.contains_key("status"));
                    assert!(kv_data.contains_key("processed_items"));
                }
                _ => panic!("Expected KeyValue payload"),
            }
        }
        _ => panic!("Expected Plugin event"),
    }
}

#[tokio::test]
async fn test_arc_sharing_efficiency() {
    // Test that Arc sharing works efficiently for data export
    let mut notification_manager = AsyncNotificationManager::new();

    // Create two subscribers
    let receiver1_result = notification_manager.subscribe(
        "test-subscriber-arc-1".to_string(),
        EventFilter::PluginOnly,
        "test-source".to_string(),
    );
    let receiver2_result = notification_manager.subscribe(
        "test-subscriber-arc-2".to_string(),
        EventFilter::PluginOnly,
        "test-source".to_string(),
    );

    assert!(receiver1_result.is_ok());
    assert!(receiver2_result.is_ok());

    let mut receiver1 = receiver1_result.unwrap();
    let mut receiver2 = receiver2_result.unwrap();

    // Create large data export payload
    let plugin_id = "memory-test-plugin".to_string();
    let scan_id = "memory-test-scan".to_string();

    let large_data = "x".repeat(10000); // 10KB of data
    let data_export = Arc::new(
        PluginDataExport::builder(plugin_id.clone(), scan_id.clone())
            .payload(DataPayload::raw(large_data, Some("text/plain".to_string())))
            .build()
            .unwrap(),
    );

    let original_arc = data_export.clone();

    // Publish event with large data
    let data_event = Event::Plugin(PluginEvent::with_data_export(
        PluginEventType::DataReady,
        plugin_id.clone(),
        scan_id.clone(),
        data_export,
    ));

    notification_manager.publish(data_event).await.unwrap();

    // Receive from both subscribers
    let event1_result = timeout(Duration::from_millis(100), receiver1.recv()).await;
    let event2_result = timeout(Duration::from_millis(100), receiver2.recv()).await;

    assert!(event1_result.is_ok());
    assert!(event2_result.is_ok());

    // Verify Arc sharing (same memory location)
    if let (Some(Event::Plugin(event1)), Some(Event::Plugin(event2))) =
        (event1_result.unwrap(), event2_result.unwrap())
    {
        let data1 = event1.data_export.as_ref().unwrap();
        let data2 = event2.data_export.as_ref().unwrap();

        // Verify they share the same Arc reference (efficient memory usage)
        assert!(Arc::ptr_eq(data1, data2));
        assert!(Arc::ptr_eq(data1, &original_arc));
    } else {
        panic!("Expected Plugin events with data");
    }
}
