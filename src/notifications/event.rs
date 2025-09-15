//! Event types for the notification system

use std::sync::Arc;
use std::time::SystemTime;

use crate::plugin::data_export::PluginDataExport;

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum ScanEventType {
    Started,
    Progress,
    DataReady,
    Warning,
    Error,
    Completed,
    Terminated,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum QueueEventType {
    Started,
    Shutdown,
    MessageAdded,
    QueueEmpty,
    MemoryLow,
    MemoryNormal,
    Terminated,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum PluginEventType {
    Registered,
    Processing,
    DataReady,
    DataComplete,
    Completed,
    Error,
    Unregistered,
    KeepAlive,
    Terminated,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum SystemEventType {
    Startup,
    Shutdown,
    ForceShutdown,
    ShutdownTimeout,
}

/// Individual event types that can be published
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ScanEvent {
    pub event_type: ScanEventType,
    pub timestamp: SystemTime,
    pub scan_id: String,
    pub message: Option<String>,
}

#[allow(dead_code)]
impl ScanEvent {
    pub fn new(event_type: ScanEventType, scan_id: String) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            scan_id,
            message: None,
        }
    }

    pub fn with_message(event_type: ScanEventType, scan_id: String, message: String) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            scan_id,
            message: Some(message),
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct QueueEvent {
    pub event_type: QueueEventType,
    pub timestamp: SystemTime,
    pub queue_id: String,
    pub size: Option<usize>,
    pub message: Option<String>,
}

#[allow(dead_code)]
impl QueueEvent {
    pub fn new(event_type: QueueEventType, queue_id: String) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            queue_id,
            size: None,
            message: None,
        }
    }

    pub fn with_size(event_type: QueueEventType, queue_id: String, size: usize) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            queue_id,
            size: Some(size),
            message: None,
        }
    }

    pub fn with_message(event_type: QueueEventType, queue_id: String, message: String) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            queue_id,
            size: None,
            message: Some(message),
        }
    }

    pub fn with_size_and_message(
        event_type: QueueEventType,
        queue_id: String,
        size: usize,
        message: String,
    ) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            queue_id,
            size: Some(size),
            message: Some(message),
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct PluginEvent {
    pub event_type: PluginEventType,
    pub timestamp: SystemTime,
    pub plugin_id: String,
    pub scan_id: String,
    pub message: Option<String>,
    pub data_export: Option<Arc<PluginDataExport>>,
}

#[allow(dead_code)]
impl PluginEvent {
    pub fn new(event_type: PluginEventType, plugin_id: String, scan_id: String) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            plugin_id,
            scan_id,
            message: None,
            data_export: None,
        }
    }

    pub fn with_message(
        event_type: PluginEventType,
        plugin_id: String,
        scan_id: String,
        message: String,
    ) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            plugin_id,
            scan_id,
            message: Some(message),
            data_export: None,
        }
    }

    pub fn with_data_export(
        event_type: PluginEventType,
        plugin_id: String,
        scan_id: String,
        data_export: Arc<PluginDataExport>,
    ) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            plugin_id,
            scan_id,
            message: None,
            data_export: Some(data_export),
        }
    }

    pub fn with_data_export_and_message(
        event_type: PluginEventType,
        plugin_id: String,
        scan_id: String,
        data_export: Arc<PluginDataExport>,
        message: String,
    ) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            plugin_id,
            scan_id,
            message: Some(message),
            data_export: Some(data_export),
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SystemEvent {
    pub event_type: SystemEventType,
    pub timestamp: SystemTime,
    pub message: Option<String>,
}

#[allow(dead_code)]
impl SystemEvent {
    pub fn new(event_type: SystemEventType) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            message: None,
        }
    }

    pub fn with_message(event_type: SystemEventType, message: String) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            message: Some(message),
        }
    }
}

/// Unified event enum that encompasses all event types
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Event {
    Scan(ScanEvent),
    Queue(QueueEvent),
    Plugin(PluginEvent),
    System(SystemEvent),
}

/// Event filtering options for subscribers
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum EventFilter {
    ScanOnly,
    QueueOnly,
    PluginOnly,
    SystemOnly,
    ScanAndPlugin,
    QueueAndSystem,
    All,
}

#[allow(dead_code)]
impl EventFilter {
    /// Check if an event should be accepted by this filter
    pub fn accepts(&self, event: &Event) -> bool {
        matches!(
            (self, event),
            (EventFilter::ScanOnly, Event::Scan(_))
                | (EventFilter::QueueOnly, Event::Queue(_))
                | (EventFilter::PluginOnly, Event::Plugin(_))
                | (EventFilter::SystemOnly, Event::System(_))
                | (EventFilter::ScanAndPlugin, Event::Scan(_))
                | (EventFilter::ScanAndPlugin, Event::Plugin(_))
                | (EventFilter::QueueAndSystem, Event::Queue(_))
                | (EventFilter::QueueAndSystem, Event::System(_))
                | (EventFilter::All, _)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_event_type_equality() {
        assert_eq!(
            SystemEventType::ForceShutdown,
            SystemEventType::ForceShutdown
        );
        assert_eq!(
            SystemEventType::ShutdownTimeout,
            SystemEventType::ShutdownTimeout
        );
        assert_ne!(
            SystemEventType::ForceShutdown,
            SystemEventType::ShutdownTimeout
        );
        assert_ne!(SystemEventType::ForceShutdown, SystemEventType::Shutdown);
    }

    #[test]
    fn test_plugin_event_type_terminated() {
        assert_eq!(PluginEventType::Terminated, PluginEventType::Terminated);
        assert_ne!(PluginEventType::Terminated, PluginEventType::Completed);
        assert_ne!(PluginEventType::Terminated, PluginEventType::Error);
    }

    #[test]
    fn test_scan_event_type_terminated() {
        assert_eq!(ScanEventType::Terminated, ScanEventType::Terminated);
        assert_ne!(ScanEventType::Terminated, ScanEventType::Completed);
        assert_ne!(ScanEventType::Terminated, ScanEventType::Error);
    }

    #[test]
    fn test_queue_event_type_terminated() {
        assert_eq!(QueueEventType::Terminated, QueueEventType::Terminated);
        assert_ne!(QueueEventType::Terminated, QueueEventType::Shutdown);
        assert_ne!(QueueEventType::Terminated, QueueEventType::QueueEmpty);
    }

    #[test]
    fn test_system_event_creation() {
        let shutdown_event = SystemEvent::new(SystemEventType::ForceShutdown);
        assert_eq!(shutdown_event.event_type, SystemEventType::ForceShutdown);
        assert!(shutdown_event.message.is_none());

        let timeout_event = SystemEvent::with_message(
            SystemEventType::ShutdownTimeout,
            "Component failed to stop in time".to_string(),
        );
        assert_eq!(timeout_event.event_type, SystemEventType::ShutdownTimeout);
        assert_eq!(
            timeout_event.message,
            Some("Component failed to stop in time".to_string())
        );
    }

    #[test]
    fn test_plugin_event_terminated_creation() {
        let event = PluginEvent::new(
            PluginEventType::Terminated,
            "output-plugin".to_string(),
            "scan-123".to_string(),
        );
        assert_eq!(event.event_type, PluginEventType::Terminated);
        assert_eq!(event.plugin_id, "output-plugin");
        assert_eq!(event.scan_id, "scan-123");
        assert!(event.message.is_none());
        assert!(event.data_export.is_none());

        let event_with_msg = PluginEvent::with_message(
            PluginEventType::Terminated,
            "dump-plugin".to_string(),
            "scan-456".to_string(),
            "Clean termination completed".to_string(),
        );
        assert_eq!(
            event_with_msg.message,
            Some("Clean termination completed".to_string())
        );
    }

    #[test]
    fn test_scan_event_terminated_creation() {
        let event = ScanEvent::new(ScanEventType::Terminated, "scan-789".to_string());
        assert_eq!(event.event_type, ScanEventType::Terminated);
        assert_eq!(event.scan_id, "scan-789");
        assert!(event.message.is_none());

        let event_with_msg = ScanEvent::with_message(
            ScanEventType::Terminated,
            "scan-101".to_string(),
            "Scanner stopped gracefully".to_string(),
        );
        assert_eq!(
            event_with_msg.message,
            Some("Scanner stopped gracefully".to_string())
        );
    }

    #[test]
    fn test_queue_event_terminated_creation() {
        let event = QueueEvent::new(QueueEventType::Terminated, "queue-202".to_string());
        assert_eq!(event.event_type, QueueEventType::Terminated);
        assert_eq!(event.queue_id, "queue-202");
        assert!(event.size.is_none());
        assert!(event.message.is_none());

        let event_with_msg = QueueEvent::with_message(
            QueueEventType::Terminated,
            "queue-303".to_string(),
            "Queue terminated cleanly".to_string(),
        );
        assert_eq!(
            event_with_msg.message,
            Some("Queue terminated cleanly".to_string())
        );
    }

    #[test]
    fn test_event_filter_accepts_new_events() {
        let system_shutdown = Event::System(SystemEvent::new(SystemEventType::ForceShutdown));
        let system_timeout = Event::System(SystemEvent::new(SystemEventType::ShutdownTimeout));
        let plugin_terminated = Event::Plugin(PluginEvent::new(
            PluginEventType::Terminated,
            "test-plugin".to_string(),
            "scan-404".to_string(),
        ));
        let scan_terminated = Event::Scan(ScanEvent::new(
            ScanEventType::Terminated,
            "scan-505".to_string(),
        ));
        let queue_terminated = Event::Queue(QueueEvent::new(
            QueueEventType::Terminated,
            "queue-606".to_string(),
        ));

        // SystemOnly filter
        let system_filter = EventFilter::SystemOnly;
        assert!(system_filter.accepts(&system_shutdown));
        assert!(system_filter.accepts(&system_timeout));
        assert!(!system_filter.accepts(&plugin_terminated));
        assert!(!system_filter.accepts(&scan_terminated));
        assert!(!system_filter.accepts(&queue_terminated));

        // PluginOnly filter
        let plugin_filter = EventFilter::PluginOnly;
        assert!(!plugin_filter.accepts(&system_shutdown));
        assert!(plugin_filter.accepts(&plugin_terminated));
        assert!(!plugin_filter.accepts(&scan_terminated));
        assert!(!plugin_filter.accepts(&queue_terminated));

        // ScanOnly filter
        let scan_filter = EventFilter::ScanOnly;
        assert!(!scan_filter.accepts(&system_shutdown));
        assert!(!scan_filter.accepts(&plugin_terminated));
        assert!(scan_filter.accepts(&scan_terminated));
        assert!(!scan_filter.accepts(&queue_terminated));

        // QueueOnly filter
        let queue_filter = EventFilter::QueueOnly;
        assert!(!queue_filter.accepts(&system_shutdown));
        assert!(!queue_filter.accepts(&plugin_terminated));
        assert!(!queue_filter.accepts(&scan_terminated));
        assert!(queue_filter.accepts(&queue_terminated));

        // QueueAndSystem filter
        let queue_system_filter = EventFilter::QueueAndSystem;
        assert!(queue_system_filter.accepts(&system_shutdown));
        assert!(queue_system_filter.accepts(&system_timeout));
        assert!(!queue_system_filter.accepts(&plugin_terminated));
        assert!(!queue_system_filter.accepts(&scan_terminated));
        assert!(queue_system_filter.accepts(&queue_terminated));

        // ScanAndPlugin filter
        let scan_plugin_filter = EventFilter::ScanAndPlugin;
        assert!(!scan_plugin_filter.accepts(&system_shutdown));
        assert!(scan_plugin_filter.accepts(&plugin_terminated));
        assert!(scan_plugin_filter.accepts(&scan_terminated));
        assert!(!scan_plugin_filter.accepts(&queue_terminated));

        // All filter
        let all_filter = EventFilter::All;
        assert!(all_filter.accepts(&system_shutdown));
        assert!(all_filter.accepts(&system_timeout));
        assert!(all_filter.accepts(&plugin_terminated));
        assert!(all_filter.accepts(&scan_terminated));
        assert!(all_filter.accepts(&queue_terminated));
    }

    #[test]
    fn test_event_debug_formatting() {
        let shutdown_event = SystemEvent::new(SystemEventType::ForceShutdown);
        let debug_str = format!("{:?}", shutdown_event);
        assert!(debug_str.contains("ForceShutdown"));

        let plugin_event = PluginEvent::new(
            PluginEventType::Terminated,
            "test-plugin".to_string(),
            "scan-123".to_string(),
        );
        let debug_str = format!("{:?}", plugin_event);
        assert!(debug_str.contains("Terminated"));
        assert!(debug_str.contains("test-plugin"));
    }
}
