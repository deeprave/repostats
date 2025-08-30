//! Tests for QueueConsumer functionality

#[cfg(test)]
mod tests {
    use crate::queue::api::{Message, QueueManager};
    use std::sync::Arc;

    #[test]
    fn test_consumer_reads_from_global_queue() {
        let manager = Arc::new(QueueManager::new());

        // Create consumer and publisher
        let consumer = manager.create_consumer("test-plugin".to_string()).unwrap();
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Publish a message
        let message = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "test_file.rs".to_string(),
        );
        let sequence = publisher.publish(message.clone()).unwrap();
        assert_eq!(sequence, 1);

        // Consumer should be able to read the message
        let read_message = consumer.read().unwrap();
        assert!(read_message.is_some());

        let received = read_message.unwrap();
        assert_eq!(received.data, "test_file.rs");
        assert_eq!(received.header.producer_id, "test-producer");

        // Subsequent read should return None (no more messages)
        let next_read = consumer.read().unwrap();
        assert!(next_read.is_none());
    }

    #[test]
    fn test_consumer_reads_messages_in_sequence_order() {
        let manager = Arc::new(QueueManager::new());

        let consumer = manager.create_consumer("test-plugin".to_string()).unwrap();
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Publish multiple messages
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

        publisher.publish(msg1).unwrap();
        publisher.publish(msg2).unwrap();
        publisher.publish(msg3).unwrap();

        // Consumer should read in sequence order
        let read1 = consumer.read().unwrap().unwrap();
        let read2 = consumer.read().unwrap().unwrap();
        let read3 = consumer.read().unwrap().unwrap();

        assert_eq!(read1.data, "file1.rs");
        assert_eq!(read2.data, "file2.rs");
        assert_eq!(read3.data, "file3.rs");

        // No more messages
        assert!(consumer.read().unwrap().is_none());
    }

    #[test]
    fn test_multiple_consumers_independent_positions() {
        let manager = Arc::new(QueueManager::new());

        let consumer1 = manager.create_consumer("plugin-a".to_string()).unwrap();
        let consumer2 = manager.create_consumer("plugin-b".to_string()).unwrap();
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .unwrap();

        // Publish messages
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

        publisher.publish(msg1).unwrap();
        publisher.publish(msg2).unwrap();

        // Consumer1 reads first message
        let read1 = consumer1.read().unwrap().unwrap();
        assert_eq!(read1.data, "file1.rs");

        // Consumer2 should also be able to read first message (independent position)
        let read2 = consumer2.read().unwrap().unwrap();
        assert_eq!(read2.data, "file1.rs");

        // Consumer1 reads second message
        let read1_next = consumer1.read().unwrap().unwrap();
        assert_eq!(read1_next.data, "file2.rs");

        // Consumer2 should still be able to read second message
        let read2_next = consumer2.read().unwrap().unwrap();
        assert_eq!(read2_next.data, "file2.rs");

        // Both consumers should now have no more messages
        assert!(consumer1.read().unwrap().is_none());
        assert!(consumer2.read().unwrap().is_none());
    }

    #[test]
    fn test_consumer_reads_messages_from_different_producers() {
        let manager = Arc::new(QueueManager::new());

        let consumer = manager
            .create_consumer("multi-producer-plugin".to_string())
            .unwrap();
        let publisher1 = manager.create_publisher("producer-a".to_string()).unwrap();
        let publisher2 = manager.create_publisher("producer-b".to_string()).unwrap();

        // Publish from different producers to the same global queue
        let msg_a = Message::new(
            "producer-a".to_string(),
            "file".to_string(),
            "from_producer_a.rs".to_string(),
        );
        let msg_b = Message::new(
            "producer-b".to_string(),
            "file".to_string(),
            "from_producer_b.rs".to_string(),
        );

        publisher1.publish(msg_a).unwrap();
        publisher2.publish(msg_b).unwrap();

        // Consumer should read both messages in global sequence order
        let read1 = consumer.read().unwrap().unwrap();
        let read2 = consumer.read().unwrap().unwrap();

        // First message should be from producer-a (published first)
        assert_eq!(read1.data, "from_producer_a.rs");
        assert_eq!(read1.header.producer_id, "producer-a");

        // Second message should be from producer-b (published second)
        assert_eq!(read2.data, "from_producer_b.rs");
        assert_eq!(read2.header.producer_id, "producer-b");
    }

    #[test]
    fn test_consumer_registration_with_global_queue() {
        let manager = Arc::new(QueueManager::new());

        // Create consumer - should register with global queue
        let consumer = manager.create_consumer("test-plugin".to_string()).unwrap();

        // Verify consumer is registered in the global queue
        let queue = manager.get_global_queue().unwrap();

        // Extract consumer ID from the consumer (need to access internal ID)
        let consumer_id = consumer.internal_consumer_id();

        // Queue should show this consumer is registered
        assert!(queue.has_consumer(consumer_id));
    }
}
