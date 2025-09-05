//! Tests for QueuePublisher functionality

#[cfg(test)]
mod tests {
    use crate::queue::api::{Message, QueueManager};
    use std::sync::Arc;

    #[test]
    fn test_publisher_publishes_to_global_queue() {
        let manager = Arc::new(QueueManager::new());

        // Create a publisher for a specific producer
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Create a test message
        let message = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "test_file.rs".to_string(),
        );

        // Publish the message
        let sequence = publisher.publish(message.clone()).unwrap();

        // Verify the message was added to the single global queue
        assert_eq!(sequence, 1); // First message should get sequence 1

        // Get the single global queue and verify the message is there
        let queue = manager.get_global_queue().unwrap();
        assert_eq!(queue.size().unwrap(), 1);
        assert_eq!(queue.head_sequence().unwrap(), 2); // Next sequence should be 2

        // There should be only one queue total
        assert_eq!(manager.queue_count(), 1);
    }

    #[test]
    fn test_publisher_with_multiple_messages_global_sequence() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Publish multiple messages to global queue
        let msg1 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        );
        let msg2 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file2.rs".to_string(),
        );
        let msg3 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file3.rs".to_string(),
        );

        let seq1 = publisher.publish(msg1).unwrap();
        let seq2 = publisher.publish(msg2).unwrap();
        let seq3 = publisher.publish(msg3).unwrap();

        // Verify global sequences are monotonic
        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(seq3, 3);

        // Verify single global queue has all messages
        let queue = manager.get_global_queue().unwrap();
        assert_eq!(queue.size().unwrap(), 3);
        assert_eq!(manager.queue_count(), 1); // Still only one queue
    }

    #[test]
    fn test_multiple_publishers_different_producers_same_global_queue() {
        let manager = Arc::new(QueueManager::new());

        // Create publishers for different producers - both use same global queue
        let publisher1 = manager.create_publisher("producer-a".to_string()).unwrap();
        let publisher2 = manager.create_publisher("producer-b".to_string()).unwrap();

        // Both publish to the same global queue
        let msg1 = Message::new(
            "producer-a".to_string(),
            "file".to_string(),
            "from_producer_a.rs".to_string(),
        );
        let msg2 = Message::new(
            "producer-b".to_string(),
            "file".to_string(),
            "from_producer_b.rs".to_string(),
        );

        let seq1 = publisher1.publish(msg1).unwrap();
        let seq2 = publisher2.publish(msg2).unwrap();

        // Global sequences should be consecutive
        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);

        // Both messages in the single global queue
        let queue = manager.get_global_queue().unwrap();
        assert_eq!(queue.size().unwrap(), 2);
        assert_eq!(manager.queue_count(), 1); // Still only one queue
    }

    #[test]
    fn test_publishers_different_producers_single_global_queue() {
        let manager = Arc::new(QueueManager::new());

        // Create publishers for different producers
        let publisher1 = manager.create_publisher("producer-1".to_string()).unwrap();
        let publisher2 = manager.create_publisher("producer-2".to_string()).unwrap();

        // Publish to the SAME global queue (different producer_ids)
        let msg1 = Message::new(
            "producer-1".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        );
        let msg2 = Message::new(
            "producer-2".to_string(),
            "file".to_string(),
            "file2.rs".to_string(),
        );

        let seq1 = publisher1.publish(msg1).unwrap();
        let seq2 = publisher2.publish(msg2).unwrap();

        // Should get consecutive global sequences (single queue)
        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);

        // There should be only ONE queue with both messages
        assert_eq!(manager.queue_count(), 1);
        assert_eq!(manager.total_message_count().unwrap(), 2);

        // The single global queue should have 2 messages
        let queue = manager.get_global_queue().unwrap();
        assert_eq!(queue.size().unwrap(), 2);
    }
}
