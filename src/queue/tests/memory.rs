//! Tests for memory management functionality

#[cfg(test)]
mod tests {
    use crate::queue::api::{Message, QueueManager};
    use std::sync::Arc;

    #[test]
    fn test_memory_monitoring_reports_queue_memory_usage() {
        let manager = Arc::new(QueueManager::new());

        // Get initial memory usage
        let initial_usage = manager.memory_usage_bytes();
        assert_eq!(initial_usage, 0); // Empty queue should report 0 bytes

        // Create publisher and add messages
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Publish several messages of known size
        let message = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "test_file.rs".to_string(),
        );

        publisher.publish(message.clone()).unwrap();
        publisher.publish(message.clone()).unwrap();
        publisher.publish(message).unwrap();

        // Memory usage should now be greater than 0
        let current_usage = manager.memory_usage_bytes();
        assert!(current_usage > 0);
        assert!(current_usage > initial_usage);

        // Memory usage should include message data and Arc overhead
        // This is an approximate check - exact size depends on implementation
        assert!(current_usage >= 100); // At least some reasonable minimum
    }

    #[test]
    fn test_memory_monitoring_tracks_message_count_and_size() {
        let manager = Arc::new(QueueManager::new());

        // Get initial stats
        let initial_stats = manager.memory_stats().unwrap();
        assert_eq!(initial_stats.total_messages, 0);
        assert_eq!(initial_stats.total_bytes, 0);

        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Add messages and verify stats update
        let msg1 = Message::new(
            "producer1".to_string(),
            "file".to_string(),
            "small.rs".to_string(),
        );
        let msg2 = Message::new(
            "producer2".to_string(),
            "file".to_string(),
            "larger_filename.rs".to_string(),
        );

        publisher.publish(msg1).unwrap();
        let stats_after_one = manager.memory_stats().unwrap();
        assert_eq!(stats_after_one.total_messages, 1);
        assert!(stats_after_one.total_bytes > 0);

        publisher.publish(msg2).unwrap();
        let stats_after_two = manager.memory_stats().unwrap();
        assert_eq!(stats_after_two.total_messages, 2);
        assert!(stats_after_two.total_bytes > stats_after_one.total_bytes);
    }

    #[test]
    fn test_garbage_collection_removes_messages_read_by_all_consumers() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Create two consumers
        let consumer1 = manager.create_consumer("plugin-1".to_string()).unwrap();
        let consumer2 = manager.create_consumer("plugin-2".to_string()).unwrap();

        // Publish messages
        let msg1 = Message::new(
            "producer".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        );
        let msg2 = Message::new(
            "producer".to_string(),
            "file".to_string(),
            "file2.rs".to_string(),
        );
        let msg3 = Message::new(
            "producer".to_string(),
            "file".to_string(),
            "file3.rs".to_string(),
        );

        publisher.publish(msg1).unwrap();
        publisher.publish(msg2).unwrap();
        publisher.publish(msg3).unwrap();

        // Initial state: 3 messages
        assert_eq!(manager.total_message_count().unwrap(), 3);

        // Consumer 1 reads first two messages
        consumer1.read().unwrap();
        consumer1.read().unwrap();

        // Consumer 2 reads only first message
        consumer2.read().unwrap();

        // Run garbage collection - should remove only message 1 (read by both consumers)
        let collected = manager.collect_garbage().unwrap();
        assert_eq!(collected, 1); // One message collected

        // Should have 2 messages remaining (message 2 not read by consumer2, message 3 not read by either)
        assert_eq!(manager.total_message_count().unwrap(), 2);

        // Consumer 2 reads message 2
        consumer2.read().unwrap();

        // Run garbage collection again - should remove message 2
        let collected = manager.collect_garbage().unwrap();
        assert_eq!(collected, 1);
        assert_eq!(manager.total_message_count().unwrap(), 1);
    }

    #[test]
    fn test_garbage_collection_with_no_consumers_removes_nothing() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Publish messages without any consumers
        publisher
            .publish(Message::new(
                "producer".to_string(),
                "file".to_string(),
                "file1.rs".to_string(),
            ))
            .unwrap();
        publisher
            .publish(Message::new(
                "producer".to_string(),
                "file".to_string(),
                "file2.rs".to_string(),
            ))
            .unwrap();

        assert_eq!(manager.total_message_count().unwrap(), 2);

        // Garbage collection with no consumers should remove nothing
        let collected = manager.collect_garbage().unwrap();
        assert_eq!(collected, 0);
        assert_eq!(manager.total_message_count().unwrap(), 2);
    }

    #[test]
    fn test_automatic_garbage_collection_on_memory_pressure() {
        let manager = Arc::new(QueueManager::new());

        // Configure low memory threshold for testing
        manager.set_memory_threshold_bytes(1000).unwrap();

        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();
        let consumer = manager.create_consumer("plugin".to_string()).unwrap();

        // Publish many messages to trigger memory pressure
        for i in 0..50 {
            let msg = Message::new(
                "producer".to_string(),
                "file".to_string(),
                format!("large_file_with_long_name_{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        let initial_count = manager.total_message_count().unwrap();
        assert_eq!(initial_count, 50);

        // Read first 25 messages
        for _ in 0..25 {
            consumer.read().unwrap();
        }

        // Publishing more messages should trigger automatic garbage collection
        let large_msg = Message::new(
            "producer".to_string(),
            "file".to_string(),
            "x".repeat(500), // Large message to trigger memory threshold
        );
        publisher.publish(large_msg).unwrap();

        // Queue should have automatically collected garbage
        let final_count = manager.total_message_count().unwrap();
        assert!(final_count < initial_count); // Should have cleaned up some messages
        assert!(final_count >= 26); // Should have at least 25 unread + 1 new message
    }

    #[test]
    fn test_memory_pressure_detection_without_threshold() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager.create_publisher("test".to_string()).unwrap();

        // Publish messages without setting threshold
        for i in 0..10 {
            let msg = Message::new(
                "producer".to_string(),
                "file".to_string(),
                format!("file{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Check memory pressure - should return false (no threshold set)
        let pressure_detected = manager.check_memory_pressure().unwrap();
        assert!(!pressure_detected);
    }

    #[test]
    fn test_memory_pressure_detection_below_threshold() {
        let manager = Arc::new(QueueManager::new());

        // Set very high threshold
        manager.set_memory_threshold_bytes(1_000_000).unwrap(); // 1MB

        let publisher = manager.create_publisher("test".to_string()).unwrap();

        // Publish a few small messages
        for i in 0..5 {
            let msg = Message::new(
                "producer".to_string(),
                "file".to_string(),
                format!("file{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Check memory pressure - should return false (below threshold)
        let pressure_detected = manager.check_memory_pressure().unwrap();
        assert!(!pressure_detected);
    }

    #[test]
    fn test_memory_pressure_detection_above_threshold() {
        let manager = Arc::new(QueueManager::new());

        // Set very low threshold
        manager.set_memory_threshold_bytes(100).unwrap(); // 100 bytes

        let publisher = manager.create_publisher("test".to_string()).unwrap();
        let _consumer = manager.create_consumer("plugin".to_string()).unwrap(); // Need consumer for garbage collection

        // Publish messages that exceed threshold
        for i in 0..10 {
            let large_msg = Message::new(
                "producer".to_string(),
                "file".to_string(),
                format!("very_long_filename_to_exceed_threshold_{}.rs", i),
            );
            publisher.publish(large_msg).unwrap();
        }

        // Check memory pressure - should return true (above threshold)
        let pressure_detected = manager.check_memory_pressure().unwrap();
        assert!(pressure_detected);
    }
}
