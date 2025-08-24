//! Traits for the notification system

use crate::notifications::event::Event;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Instant;

/// Statistics tracking for a subscriber
#[allow(dead_code)]
pub struct SubscriberStatistics {
    queue_size: AtomicUsize,
    messages_processed: AtomicUsize,
    error_count: AtomicUsize,
    last_message_time: RwLock<Option<Instant>>,
    last_error_time: RwLock<Option<Instant>>,
    last_error_log_time: RwLock<Option<Instant>>,
}

impl Default for SubscriberStatistics {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl SubscriberStatistics {
    pub fn new() -> Self {
        Self {
            queue_size: AtomicUsize::new(0),
            messages_processed: AtomicUsize::new(0),
            error_count: AtomicUsize::new(0),
            last_message_time: RwLock::new(None),
            last_error_time: RwLock::new(None),
            last_error_log_time: RwLock::new(None),
        }
    }

    pub fn queue_size(&self) -> usize {
        self.queue_size.load(Ordering::Relaxed)
    }

    pub fn increment_queue_size(&self) {
        self.queue_size.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_queue_size(&self) {
        self.queue_size
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                if current == 0 {
                    Some(0)
                } else {
                    Some(current - 1)
                }
            })
            .ok();
    }

    pub fn messages_processed(&self) -> usize {
        self.messages_processed.load(Ordering::Relaxed)
    }

    pub fn record_message_processed(&self) {
        self.messages_processed.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut time) = self.last_message_time.write() {
            *time = Some(Instant::now());
        }
    }

    pub fn error_count(&self) -> usize {
        self.error_count.load(Ordering::Relaxed)
    }

    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut time) = self.last_error_time.write() {
            *time = Some(Instant::now());
        }
    }

    pub fn record_error_logged(&self) {
        if let Ok(mut time) = self.last_error_log_time.write() {
            *time = Some(Instant::now());
        }
    }

    pub fn last_message_time(&self) -> Option<Instant> {
        *self.last_message_time.read().ok()?
    }

    pub fn last_error_time(&self) -> Option<Instant> {
        *self.last_error_time.read().ok()?
    }

    pub fn last_error_log_time(&self) -> Option<Instant> {
        *self.last_error_log_time.read().ok()?
    }
}

/// Trait for event subscribers
#[async_trait]
#[allow(dead_code)]
pub trait Subscriber: Send + Sync {
    /// Handle an incoming event
    async fn handle_event(&self, event: Event) -> Result<(), Box<dyn std::error::Error>>;

    /// Get the unique identifier for this subscriber
    fn subscriber_id(&self) -> &str;

    /// Get the source identifier for debugging
    fn source(&self) -> &str;

    /// Get statistics for this subscriber
    fn get_statistics(&self) -> &SubscriberStatistics;
}
