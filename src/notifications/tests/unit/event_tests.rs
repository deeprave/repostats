//! Unit tests for Event enum and EventFilter

use crate::notifications::event::{
    Event, PluginEvent, PluginEventType, QueueEvent, QueueEventType, ScanEvent, ScanEventType,
    SystemEvent, SystemEventType,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_enum_variants() {
        let scan_event = ScanEvent::with_message(
            ScanEventType::Started,
            "test_scan".to_string(),
            "scan started".to_string(),
        );

        let queue_event =
            QueueEvent::with_size(QueueEventType::MessageAdded, "test_queue".to_string(), 100);

        let plugin_event = PluginEvent::with_message(
            PluginEventType::Processing,
            "test_plugin".to_string(),
            "processing".to_string(),
        );

        let system_event =
            SystemEvent::with_message(SystemEventType::Startup, "application started".to_string());

        // Test Event enum construction
        let _scan = Event::Scan(scan_event);
        let _queue = Event::Queue(queue_event);
        let _plugin = Event::Plugin(plugin_event);
        let _system = Event::System(system_event);
    }

    #[test]
    fn test_event_enum_cloning() {
        let scan_event = ScanEvent::with_message(
            ScanEventType::Started,
            "test_scan".to_string(),
            "scan started".to_string(),
        );

        let event = Event::Scan(scan_event);
        let cloned_event = event.clone();

        match (event, cloned_event) {
            (Event::Scan(original), Event::Scan(cloned)) => {
                assert_eq!(original.scan_id, cloned.scan_id);
                assert_eq!(original.message, cloned.message);
            }
            _ => panic!("Event cloning failed"),
        }
    }

    #[test]
    fn test_event_filter_enum_variants() {
        use crate::notifications::event::EventFilter;

        // Test EventFilter enum construction
        let _scan_only = EventFilter::ScanOnly;
        let _queue_only = EventFilter::QueueOnly;
        let _plugin_only = EventFilter::PluginOnly;
        let _system_only = EventFilter::SystemOnly;
        let _scan_and_plugin = EventFilter::ScanAndPlugin;
        let _queue_and_system = EventFilter::QueueAndSystem;
        let _all = EventFilter::All;
    }

    #[test]
    fn test_event_filter_accepts_logic() {
        use crate::notifications::event::EventFilter;

        let scan_event = Event::Scan(ScanEvent::with_message(
            ScanEventType::Started,
            "test_scan".to_string(),
            "scan started".to_string(),
        ));

        let queue_event = Event::Queue(QueueEvent::with_size(
            QueueEventType::MessageAdded,
            "test_queue".to_string(),
            100,
        ));

        let plugin_event = Event::Plugin(PluginEvent::with_message(
            PluginEventType::Processing,
            "test_plugin".to_string(),
            "processing".to_string(),
        ));

        let system_event = Event::System(SystemEvent::with_message(
            SystemEventType::Startup,
            "application started".to_string(),
        ));

        // Test ScanOnly filter
        assert!(EventFilter::ScanOnly.accepts(&scan_event));
        assert!(!EventFilter::ScanOnly.accepts(&queue_event));
        assert!(!EventFilter::ScanOnly.accepts(&plugin_event));
        assert!(!EventFilter::ScanOnly.accepts(&system_event));

        // Test QueueOnly filter
        assert!(!EventFilter::QueueOnly.accepts(&scan_event));
        assert!(EventFilter::QueueOnly.accepts(&queue_event));
        assert!(!EventFilter::QueueOnly.accepts(&plugin_event));
        assert!(!EventFilter::QueueOnly.accepts(&system_event));

        // Test PluginOnly filter
        assert!(!EventFilter::PluginOnly.accepts(&scan_event));
        assert!(!EventFilter::PluginOnly.accepts(&queue_event));
        assert!(EventFilter::PluginOnly.accepts(&plugin_event));
        assert!(!EventFilter::PluginOnly.accepts(&system_event));

        // Test SystemOnly filter
        assert!(!EventFilter::SystemOnly.accepts(&scan_event));
        assert!(!EventFilter::SystemOnly.accepts(&queue_event));
        assert!(!EventFilter::SystemOnly.accepts(&plugin_event));
        assert!(EventFilter::SystemOnly.accepts(&system_event));

        // Test ScanAndPlugin filter
        assert!(EventFilter::ScanAndPlugin.accepts(&scan_event));
        assert!(!EventFilter::ScanAndPlugin.accepts(&queue_event));
        assert!(EventFilter::ScanAndPlugin.accepts(&plugin_event));
        assert!(!EventFilter::ScanAndPlugin.accepts(&system_event));

        // Test QueueAndSystem filter
        assert!(!EventFilter::QueueAndSystem.accepts(&scan_event));
        assert!(EventFilter::QueueAndSystem.accepts(&queue_event));
        assert!(!EventFilter::QueueAndSystem.accepts(&plugin_event));
        assert!(EventFilter::QueueAndSystem.accepts(&system_event));

        // Test All filter
        assert!(EventFilter::All.accepts(&scan_event));
        assert!(EventFilter::All.accepts(&queue_event));
        assert!(EventFilter::All.accepts(&plugin_event));
        assert!(EventFilter::All.accepts(&system_event));
    }

    #[test]
    fn test_event_timestamp_auto_population() {
        use std::time::{Duration, SystemTime};

        let before = SystemTime::now();
        let scan_event = ScanEvent::with_message(
            ScanEventType::Started,
            "test_scan".to_string(),
            "test message".to_string(),
        );
        let after = SystemTime::now();

        // Timestamp should be between before and after
        assert!(scan_event.timestamp >= before);
        assert!(scan_event.timestamp <= after);

        // Test different event types have timestamps
        let queue_event =
            QueueEvent::with_size(QueueEventType::MessageAdded, "test_queue".to_string(), 42);

        let plugin_event = PluginEvent::new(PluginEventType::Processing, "test_plugin".to_string());

        let system_event =
            SystemEvent::with_message(SystemEventType::Startup, "startup complete".to_string());

        // All should have recent timestamps
        let now = SystemTime::now();
        let recent_threshold = Duration::from_millis(100);

        assert!(now.duration_since(queue_event.timestamp).unwrap() < recent_threshold);
        assert!(now.duration_since(plugin_event.timestamp).unwrap() < recent_threshold);
        assert!(now.duration_since(system_event.timestamp).unwrap() < recent_threshold);
    }

    #[test]
    fn test_event_constructor_variants() {
        // Test basic constructors (no message, no size)
        let scan_basic = ScanEvent::new(ScanEventType::Started, "scan1".to_string());
        assert_eq!(scan_basic.message, None);

        let queue_basic = QueueEvent::new(QueueEventType::QueueEmpty, "queue1".to_string());
        assert_eq!(queue_basic.size, None);
        assert_eq!(queue_basic.message, None);

        let plugin_basic = PluginEvent::new(PluginEventType::Registered, "plugin1".to_string());
        assert_eq!(plugin_basic.message, None);

        let system_basic = SystemEvent::new(SystemEventType::Shutdown);
        assert_eq!(system_basic.message, None);

        // Test with_message constructors
        let scan_msg = ScanEvent::with_message(
            ScanEventType::Progress,
            "scan2".to_string(),
            "50% complete".to_string(),
        );
        assert_eq!(scan_msg.message, Some("50% complete".to_string()));

        // Test QueueEvent special constructors
        let queue_size =
            QueueEvent::with_size(QueueEventType::MessageAdded, "queue2".to_string(), 250);
        assert_eq!(queue_size.size, Some(250));
        assert_eq!(queue_size.message, None);

        let queue_msg = QueueEvent::with_message(
            QueueEventType::MessageAdded,
            "queue3".to_string(),
            "processed successfully".to_string(),
        );
        assert_eq!(queue_msg.size, None);
        assert_eq!(
            queue_msg.message,
            Some("processed successfully".to_string())
        );

        let queue_full = QueueEvent::with_size_and_message(
            QueueEventType::MessageAdded,
            "queue4".to_string(),
            500,
            "batch added".to_string(),
        );
        assert_eq!(queue_full.size, Some(500));
        assert_eq!(queue_full.message, Some("batch added".to_string()));
    }
}
