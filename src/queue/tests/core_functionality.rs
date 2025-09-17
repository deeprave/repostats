//! Core Functionality Tests - Verify Essential Queue Operations
//!
//! These tests verify that core queue functionality remains intact
//! after removing over-engineered monitoring and management features.

#[cfg(test)]
mod tests {
    use crate::queue::api::{Message, QueueManager};
    use std::sync::Arc;

    #[test]
    fn test_core_publish_consume_workflow() {
        // TDD Test: This test verifies core functionality that MUST remain after cleanup
        let manager = Arc::new(QueueManager::new());

        // Create consumer and publisher
        let consumer = manager.create_consumer("test-plugin".to_string()).unwrap();
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Core functionality: Publish a message
        let message = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "test_file.rs".to_string(),
        );
        let sequence = publisher.publish(message.clone()).unwrap();
        assert_eq!(sequence, 1);

        // Core functionality: Consumer reads the message
        let read_message = consumer.read().unwrap();
        assert!(read_message.is_some());

        let received = read_message.unwrap();
        assert_eq!(received.data, "test_file.rs");
        assert_eq!(received.header.producer_id, "test-producer");

        // Core functionality: No more messages available
        let next_read = consumer.read().unwrap();
        assert!(next_read.is_none());
    }

    #[test]
    fn test_core_multiple_consumers() {
        // TDD Test: Multiple consumers should work independently
        let manager = Arc::new(QueueManager::new());

        // Create multiple consumers and one publisher
        let consumer1 = manager.create_consumer("plugin-1".to_string()).unwrap();
        let consumer2 = manager.create_consumer("plugin-2".to_string()).unwrap();
        let publisher = manager.create_publisher("producer".to_string()).unwrap();

        // Publish messages
        let msg1 = Message::new(
            "producer".to_string(),
            "type1".to_string(),
            "data1".to_string(),
        );
        let msg2 = Message::new(
            "producer".to_string(),
            "type2".to_string(),
            "data2".to_string(),
        );

        publisher.publish(msg1).unwrap();
        publisher.publish(msg2).unwrap();

        // Both consumers should be able to read all messages
        let c1_msg1 = consumer1.read().unwrap().unwrap();
        let c1_msg2 = consumer1.read().unwrap().unwrap();

        let c2_msg1 = consumer2.read().unwrap().unwrap();
        let c2_msg2 = consumer2.read().unwrap().unwrap();

        // Verify both consumers got the same messages
        assert_eq!(c1_msg1.data, "data1");
        assert_eq!(c1_msg2.data, "data2");
        assert_eq!(c2_msg1.data, "data1");
        assert_eq!(c2_msg2.data, "data2");
    }

    #[test]
    fn test_core_empty_queue_behavior() {
        // TDD Test: Reading from empty queue should return None
        let manager = Arc::new(QueueManager::new());
        let consumer = manager.create_consumer("test-plugin".to_string()).unwrap();

        // Reading from empty queue should return None, not error
        let result = consumer.read().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_core_basic_error_handling() {
        // TDD Test: Basic error conditions should be handled gracefully
        let manager = Arc::new(QueueManager::new());

        // Creating consumer and publisher should succeed
        let consumer_result = manager.create_consumer("test".to_string());
        assert!(consumer_result.is_ok());

        let publisher_result = manager.create_publisher("test".to_string());
        assert!(publisher_result.is_ok());
    }

    #[test]
    fn test_core_queue_manager_creation() {
        // TDD Test: QueueManager should be creatable and functional
        let manager = QueueManager::new();

        // Should be able to wrap in Arc for sharing
        let shared_manager = Arc::new(manager);

        // Should be able to create components
        let consumer = shared_manager.create_consumer("test".to_string()).unwrap();
        let publisher = shared_manager.create_publisher("test".to_string()).unwrap();

        // Components should be usable
        let message = Message::new("test".to_string(), "test".to_string(), "test".to_string());
        let _sequence = publisher.publish(message).unwrap();
        let _read_result = consumer.read().unwrap();
    }
}
