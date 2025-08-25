//! Tests for Queue Module

use super::*;

#[cfg(test)]
mod queue_manager_tests {
    use super::*;

    #[test]
    fn test_queue_manager_creation() {
        use std::sync::Arc;

        let manager = Arc::new(QueueManager::new());

        // Test that we can create publishers with producer_id (multiple producers support)
        let publisher1 = manager.create_publisher("producer-1".to_string()).unwrap();
        let publisher2 = manager.create_publisher("producer-2".to_string()).unwrap();

        assert_eq!(publisher1.producer_id(), "producer-1");
        assert_eq!(publisher2.producer_id(), "producer-2");

        // Test that we can create consumers
        let consumer1 = manager.create_consumer("plugin-a".to_string()).unwrap();
        let consumer2 = manager.create_consumer("plugin-b".to_string()).unwrap();

        assert_eq!(consumer1.plugin_name(), "plugin-a");
        assert_eq!(consumer2.plugin_name(), "plugin-b");

        // Consumer IDs should be unique
        assert_ne!(consumer1.consumer_id(), consumer2.consumer_id());
    }

    #[test]
    fn test_single_global_queue_management() {
        use std::sync::Arc;

        let manager = Arc::new(QueueManager::new());

        // Test initial state - single global queue always exists
        assert_eq!(manager.queue_count(), 1); // Global queue exists on creation
        assert_eq!(manager.producer_ids().len(), 1); // Returns ["global"]
        assert_eq!(manager.total_message_count(), 0); // No messages yet

        // Creating publishers doesn't create new queues - they all use the same global queue
        let _publisher1 = manager
            .create_publisher("test-producer".to_string())
            .unwrap();
        let _publisher2 = manager
            .create_publisher("another-producer".to_string())
            .unwrap();

        // Queue count remains 1 (single global queue)
        assert_eq!(manager.queue_count(), 1);

        // Can get the global queue directly
        let global_queue = manager.get_global_queue().unwrap();
        assert_eq!(global_queue.queue_id(), "global");

        // Legacy get_queue method returns the same global queue regardless of producer_id
        let queue1 = manager.get_queue("test-producer").unwrap();
        let queue2 = manager.get_queue("another-producer").unwrap();

        // Both should be the same global queue
        assert_eq!(queue1.queue_id(), "global");
        assert_eq!(queue2.queue_id(), "global");

        // Should be the same Arc instance
        assert!(Arc::ptr_eq(&queue1, &queue2));
        assert!(Arc::ptr_eq(&global_queue, &queue1));
    }
}
