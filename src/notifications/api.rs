//! Public API for the notification system
//!
//! This module provides the complete public API for the notification system.
//! External modules should import from here rather than directly from internal modules.
//! See docs/notification_system.md for complete documentation.

// Core event types and enums
pub use crate::notifications::event::{
    Event, EventFilter, PluginEvent, PluginEventType, QueueEvent, QueueEventType, ScanEvent,
    ScanEventType, SystemEvent, SystemEventType,
};

// Manager and utilities
pub use crate::notifications::error::NotificationError;
pub use crate::notifications::manager::{AsyncNotificationManager, EventReceiver};

// Traits and statistics
pub use crate::notifications::traits::{Subscriber, SubscriberStatistics};
