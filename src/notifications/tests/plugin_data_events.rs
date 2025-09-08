//! Tests for plugin event structures and serialisation with data payload support

use std::sync::Arc;

use crate::notifications::event::{Event, PluginEvent, PluginEventType};
use crate::plugin::data_export::{DataPayload, ExportHints, PluginDataExport};

#[test]
fn test_plugin_event_with_data_export_creation() {
    let plugin_id = "test_plugin".to_string();
    let scan_id = "scan_123".to_string();

    // Create test data export using builder
    let data_export = Arc::new(
        PluginDataExport::builder(plugin_id.clone(), scan_id.clone())
            .payload(DataPayload::raw("test data".to_string(), None))
            .hints(ExportHints::default())
            .build()
            .unwrap(),
    );

    let event = PluginEvent::with_data_export(
        PluginEventType::DataReady,
        plugin_id.clone(),
        scan_id.clone(),
        data_export.clone(),
    );

    assert_eq!(event.event_type, PluginEventType::DataReady);
    assert_eq!(event.plugin_id, plugin_id);
    assert_eq!(event.scan_id, scan_id);
    assert!(event.data_export.is_some());
    assert!(event.message.is_none());

    let data = event.data_export.unwrap();
    assert_eq!(data.plugin_id, plugin_id);
    assert_eq!(data.scan_id, scan_id);
}

#[test]
fn test_plugin_event_without_data_export() {
    let plugin_id = "test_plugin".to_string();
    let scan_id = "scan_123".to_string();

    let event = PluginEvent::new(
        PluginEventType::Registered,
        plugin_id.clone(),
        scan_id.clone(),
    );

    assert_eq!(event.event_type, PluginEventType::Registered);
    assert_eq!(event.plugin_id, plugin_id);
    assert_eq!(event.scan_id, scan_id);
    assert!(event.data_export.is_none());
    assert!(event.message.is_none());
}

#[test]
fn test_plugin_event_with_message_and_data_export() {
    let plugin_id = "test_plugin".to_string();
    let scan_id = "scan_123".to_string();
    let message = "Data processing complete".to_string();

    let data_export = Arc::new(
        PluginDataExport::builder(plugin_id.clone(), scan_id.clone())
            .payload(DataPayload::raw("result data".to_string(), None))
            .build()
            .unwrap(),
    );

    let event = PluginEvent::with_data_export_and_message(
        PluginEventType::DataReady,
        plugin_id.clone(),
        scan_id.clone(),
        data_export.clone(),
        message.clone(),
    );

    assert_eq!(event.event_type, PluginEventType::DataReady);
    assert_eq!(event.plugin_id, plugin_id);
    assert_eq!(event.scan_id, scan_id);
    assert!(event.data_export.is_some());
    assert_eq!(event.message, Some(message));
}

#[test]
fn test_plugin_event_clone() {
    let plugin_id = "test_plugin".to_string();
    let scan_id = "scan_123".to_string();

    let data_export = Arc::new(
        PluginDataExport::builder(plugin_id.clone(), scan_id.clone())
            .payload(DataPayload::raw("cloneable data".to_string(), None))
            .build()
            .unwrap(),
    );

    let original_event = PluginEvent::with_data_export(
        PluginEventType::DataReady,
        plugin_id.clone(),
        scan_id.clone(),
        data_export.clone(),
    );

    let cloned_event = original_event.clone();

    assert_eq!(cloned_event.event_type, original_event.event_type);
    assert_eq!(cloned_event.plugin_id, original_event.plugin_id);
    assert_eq!(cloned_event.scan_id, original_event.scan_id);
    assert_eq!(
        cloned_event.data_export.is_some(),
        original_event.data_export.is_some()
    );

    // Verify Arc sharing
    let original_data = original_event.data_export.as_ref().unwrap();
    let cloned_data = cloned_event.data_export.as_ref().unwrap();
    assert!(Arc::ptr_eq(original_data, cloned_data));
}

#[test]
fn test_plugin_event_unified_enum_with_data() {
    let plugin_id = "test_plugin".to_string();
    let scan_id = "scan_123".to_string();

    let data_export = Arc::new(
        PluginDataExport::builder(plugin_id.clone(), scan_id.clone())
            .payload(DataPayload::raw("unified event data".to_string(), None))
            .build()
            .unwrap(),
    );

    let plugin_event = PluginEvent::with_data_export(
        PluginEventType::DataReady,
        plugin_id.clone(),
        scan_id.clone(),
        data_export.clone(),
    );

    let unified_event = Event::Plugin(plugin_event);

    match unified_event {
        Event::Plugin(event) => {
            assert_eq!(event.event_type, PluginEventType::DataReady);
            assert!(event.data_export.is_some());
        }
        _ => panic!("Expected Plugin event"),
    }
}

#[test]
fn test_plugin_event_with_different_data_types() {
    let plugin_id = "test_plugin".to_string();
    let scan_id = "scan_123".to_string();

    // Test with different payload types
    let payloads = vec![
        DataPayload::raw("raw string data".to_string(), None),
        DataPayload::tabular(
            crate::plugin::data_export::DataSchema::new(
                "test_schema".to_string(),
                "1.0".to_string(),
            ),
            vec![],
        ),
        DataPayload::hierarchical(vec![]),
        DataPayload::key_value(std::collections::HashMap::new()),
    ];

    for payload in payloads {
        let data_export = Arc::new(
            PluginDataExport::builder(plugin_id.clone(), scan_id.clone())
                .payload(payload)
                .build()
                .unwrap(),
        );

        let event = PluginEvent::with_data_export(
            PluginEventType::DataReady,
            plugin_id.clone(),
            scan_id.clone(),
            data_export.clone(),
        );

        assert_eq!(event.event_type, PluginEventType::DataReady);
        assert!(event.data_export.is_some());
    }
}
