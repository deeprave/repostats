//! Public API for the notification system
//!
//! See docs/notification_system.md for complete documentation.

pub use crate::notifications::event::{
    Event, EventFilter,
    ScanEvent, ScanEventType,
    QueueEvent, QueueEventType,
    PluginEvent, PluginEventType,
    SystemEvent, SystemEventType,
};

pub use crate::notifications::traits::{
    Subscriber, SubscriberStatistics,
};

pub use crate::notifications::manager::AsyncNotificationManager;

pub use crate::notifications::error::NotificationError;