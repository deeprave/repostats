//! Event types for the notification system

use std::time::SystemTime;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ScanEventType {
    Started,
    Progress,
    DataReady,
    Warning,
    Error,
    Completed,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum QueueEventType {
    MessageAdded,
    MessageProcessed,
    QueueEmpty,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum PluginEventType {
    Registered,
    Processing,
    DataReady,
    Error,
    Unregistered,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum SystemEventType {
    Startup,
    Shutdown,
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
    pub message: Option<String>,
}

#[allow(dead_code)]
impl PluginEvent {
    pub fn new(event_type: PluginEventType, plugin_id: String) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            plugin_id,
            message: None,
        }
    }

    pub fn with_message(event_type: PluginEventType, plugin_id: String, message: String) -> Self {
        Self {
            event_type,
            timestamp: SystemTime::now(),
            plugin_id,
            message: Some(message),
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
        match (self, event) {
            (EventFilter::ScanOnly, Event::Scan(_)) => true,
            (EventFilter::QueueOnly, Event::Queue(_)) => true,
            (EventFilter::PluginOnly, Event::Plugin(_)) => true,
            (EventFilter::SystemOnly, Event::System(_)) => true,
            (EventFilter::ScanAndPlugin, Event::Scan(_)) => true,
            (EventFilter::ScanAndPlugin, Event::Plugin(_)) => true,
            (EventFilter::QueueAndSystem, Event::Queue(_)) => true,
            (EventFilter::QueueAndSystem, Event::System(_)) => true,
            (EventFilter::All, _) => true,
            _ => false,
        }
    }
}
