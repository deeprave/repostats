//! Unit tests for Subscriber trait and SubscriberStatistics

use crate::notifications::event::{Event, SystemEvent, SystemEventType};
use crate::notifications::traits::{Subscriber, SubscriberStatistics};
use async_trait::async_trait;
use std::time::Instant;

/// Mock subscriber for testing
struct MockSubscriber {
    id: String,
    source: String,
    statistics: SubscriberStatistics,
}

impl MockSubscriber {
    fn new(id: String, source: String) -> Self {
        Self {
            id,
            source,
            statistics: SubscriberStatistics::new(),
        }
    }
}

#[async_trait]
impl Subscriber for MockSubscriber {
    async fn handle_event(&self, event: Event) -> Result<(), Box<dyn std::error::Error>> {
        // Decrement queue size when processing
        self.statistics.decrement_queue_size();
        self.statistics.record_message_processed();

        // Simulate some processing
        match event {
            Event::System(SystemEvent {
                event_type: SystemEventType::Shutdown,
                ..
            }) => Err("Simulated error on shutdown".into()),
            _ => Ok(()),
        }
    }

    fn subscriber_id(&self) -> &str {
        &self.id
    }

    fn source(&self) -> &str {
        &self.source
    }

    fn get_statistics(&self) -> &SubscriberStatistics {
        &self.statistics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscriber_trait_definition() {
        let subscriber =
            MockSubscriber::new("test_subscriber".to_string(), "test:unit".to_string());

        // Test subscriber_id
        assert_eq!(subscriber.subscriber_id(), "test_subscriber");

        // Test source
        assert_eq!(subscriber.source(), "test:unit");

        // Test get_statistics
        let stats = subscriber.get_statistics();
        assert_eq!(stats.queue_size(), 0);
        assert_eq!(stats.messages_processed(), 0);
        assert_eq!(stats.error_count(), 0);
    }

    #[tokio::test]
    async fn test_subscriber_statistics_tracking() {
        let subscriber = MockSubscriber::new("stats_test".to_string(), "test:stats".to_string());

        let stats = subscriber.get_statistics();

        // Test queue size tracking
        stats.increment_queue_size();
        stats.increment_queue_size();
        assert_eq!(stats.queue_size(), 2);

        stats.decrement_queue_size();
        assert_eq!(stats.queue_size(), 1);

        // Test message processing tracking
        let event = Event::System(SystemEvent::new(SystemEventType::Startup));
        let _ = subscriber.handle_event(event).await;

        assert_eq!(stats.queue_size(), 0); // Decremented in handle_event
        assert_eq!(stats.messages_processed(), 1);

        // Test error tracking
        let shutdown_event = Event::System(SystemEvent::new(SystemEventType::Shutdown));
        let result = subscriber.handle_event(shutdown_event).await;
        assert!(result.is_err());

        // After error, we should record it
        stats.record_error();
        assert_eq!(stats.error_count(), 1);
    }

    #[tokio::test]
    async fn test_subscriber_statistics_timestamps() {
        let stats = SubscriberStatistics::new();

        // Record a message processed
        let before = Instant::now();
        stats.record_message_processed();
        let after = Instant::now();

        // Last message time should be between before and after
        let last_msg_time = stats.last_message_time();
        assert!(last_msg_time.is_some());
        let msg_time = last_msg_time.unwrap();
        assert!(msg_time >= before);
        assert!(msg_time <= after);

        // Record an error
        stats.record_error();

        // Check error timestamps
        let last_error_time = stats.last_error_time();
        assert!(last_error_time.is_some());

        let last_error_log_time = stats.last_error_log_time();
        assert!(last_error_log_time.is_none()); // Not logged yet

        stats.record_error_logged();
        let last_error_log_time = stats.last_error_log_time();
        assert!(last_error_log_time.is_some());
    }

    #[test]
    fn test_concurrent_statistics_updates() {
        use std::sync::Arc;
        use std::thread;

        let stats = Arc::new(SubscriberStatistics::new());
        let num_threads = 10;
        let operations_per_thread = 100;

        // Test concurrent queue size operations
        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let stats_clone = Arc::clone(&stats);
                thread::spawn(move || {
                    for _ in 0..operations_per_thread {
                        stats_clone.increment_queue_size();
                        stats_clone.decrement_queue_size();
                        stats_clone.record_message_processed();
                        stats_clone.record_error();
                    }
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final state - queue size should be 0 (equal increments/decrements)
        assert_eq!(stats.queue_size(), 0);

        // Messages processed should be num_threads * operations_per_thread
        assert_eq!(
            stats.messages_processed(),
            num_threads * operations_per_thread
        );

        // Error count should be num_threads * operations_per_thread
        assert_eq!(stats.error_count(), num_threads * operations_per_thread);

        // Timestamps should be set
        assert!(stats.last_message_time().is_some());
        assert!(stats.last_error_time().is_some());
    }

    #[test]
    fn test_queue_size_underflow_protection() {
        let stats = SubscriberStatistics::new();

        // Start with queue size 0
        assert_eq!(stats.queue_size(), 0);

        // Try to decrement - should not underflow
        stats.decrement_queue_size();
        assert_eq!(stats.queue_size(), 0);

        // Multiple decrements should still stay at 0
        for _ in 0..5 {
            stats.decrement_queue_size();
        }
        assert_eq!(stats.queue_size(), 0);

        // Normal operation should still work
        stats.increment_queue_size();
        assert_eq!(stats.queue_size(), 1);

        stats.decrement_queue_size();
        assert_eq!(stats.queue_size(), 0);
    }
}
