//! Error types for the notification system

use std::fmt;

#[derive(Debug, Clone)]
pub enum NotificationError {
    SubscriberNotFound(String),
    ChannelClosed(String),
    PublishFailed {
        event_type: String,
        failed_subscribers: Vec<String>,
    },
    Fatal {
        reason: String,
        context: String,
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
            NotificationError::SubscriberNotFound(id) => {
                write!(f, "Subscriber not found: {}", id)
            }
            NotificationError::ChannelClosed(id) => {
                write!(f, "Channel closed for subscriber: {}", id)
            }
            NotificationError::PublishFailed { event_type, failed_subscribers } => {
                write!(f, "Failed to publish {} event to {} subscribers: {:?}",
                       event_type, failed_subscribers.len(), failed_subscribers)
            }
            NotificationError::Fatal { reason, context } => {
                write!(f, "Fatal notification error: {} (context: {})", reason, context)
            }
            NotificationError::OutOfMemory { queue_sizes, total_events } => {
                write!(f, "Out of memory: {} total events across {} subscribers",
                       total_events, queue_sizes.len())
            }
            NotificationError::SystemOverload { active_subscribers, high_water_mark_count, stale_count } => {
                write!(f, "System overload: {} active subscribers, {} at high water mark, {} stale",
                       active_subscribers, high_water_mark_count, stale_count)
            }
        }
    }
}

impl std::error::Error for NotificationError {}