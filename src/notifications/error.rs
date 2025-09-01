//! Error types for the notification system

use std::fmt;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum NotificationError {
    ChannelClosed(String),
    ChannelFull(String),
    PublishFailed {
        event_type: String,
        failed_subscribers: Vec<String>,
    },
    OutOfMemory {
        queue_sizes: Vec<(String, usize)>,
        total_events: usize,
    },
    SystemOverload {
        active_subscribers: usize,
        high_water_mark_count: usize,
        stale_count: usize,
    },
}

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationError::ChannelClosed(id) => {
                write!(f, "Channel closed for subscriber: {id}")
            }
            NotificationError::ChannelFull(msg) => {
                write!(f, "Channel full: {msg}")
            }
            NotificationError::PublishFailed {
                event_type,
                failed_subscribers,
            } => {
                write!(
                    f,
                    "Failed to publish {} event to {} subscribers: {:?}",
                    event_type,
                    failed_subscribers.len(),
                    failed_subscribers
                )
            }
            NotificationError::OutOfMemory {
                queue_sizes,
                total_events,
            } => {
                write!(
                    f,
                    "Out of memory: {} total events across {} subscribers",
                    total_events,
                    queue_sizes.len()
                )
            }
            NotificationError::SystemOverload {
                active_subscribers,
                high_water_mark_count,
                stale_count,
            } => {
                write!(
                    f,
                    "System overload: {active_subscribers} \
                    active subscribers, {high_water_mark_count} \
                    at high-water mark, {stale_count} stale"
                )
            }
        }
    }
}

impl std::error::Error for NotificationError {}

impl crate::core::error_handling::ContextualError for NotificationError {
    fn is_user_actionable(&self) -> bool {
        false // All notification errors are system-level
    }

    fn user_message(&self) -> Option<&str> {
        None
    }
}
