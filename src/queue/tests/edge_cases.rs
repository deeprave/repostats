//! Edge case and error condition tests for the queue system
//!
//! These tests verify that the system handles various error conditions
//! gracefully and maintains consistency under extreme conditions.

#[cfg(test)]
mod tests {
    use crate::queue::api::{Message, QueueManager};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_empty_queue_edge_cases() {
        let manager = Arc::new(QueueManager::new());

        // Test reading from empty queue
        let consumer = manager
            .create_consumer("empty-test-consumer".to_string())
            .unwrap();

        // Should return None for empty queue
        let result = consumer.read().unwrap();
        assert!(
            result.is_none(),
            "Reading from empty queue should return None"
        );

        // Batch read from empty queue
        let batch = consumer.read_batch(10).unwrap();
        assert!(
            batch.is_empty(),
            "Batch read from empty queue should return empty vector"
        );

        // Acknowledge empty batch
        let ack_result = consumer.acknowledge_batch(&[]).unwrap();
        assert_eq!(ack_result, 0, "Acknowledging empty batch should return 0");

        // Test statistics on empty queue
        let memory_stats = manager.memory_stats().unwrap();
        assert_eq!(memory_stats.total_messages, 0);

        let lag_stats = manager.get_lag_statistics().unwrap();
        assert_eq!(lag_stats.total_consumers, 1); // Consumer is registered
        assert_eq!(lag_stats.max_lag, 0);
        assert_eq!(lag_stats.min_lag, 0);
        assert_eq!(lag_stats.avg_lag, 0.0);

        println!("✓ Empty queue edge cases handled correctly");
    }

    #[test]
    fn test_extremely_large_messages() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("large-message-producer".to_string())
            .unwrap();
        let consumer = manager
            .create_consumer("large-message-consumer".to_string())
            .unwrap();

        // Create a very large message (1MB)
        let large_data = "x".repeat(1024 * 1024);
        let large_message = Message::new(
            "large-message-producer".to_string(),
            "bulk_data".to_string(),
            large_data.clone(),
        );

        // Should handle large messages
        let sequence = publisher.publish(large_message).unwrap();
        assert_eq!(sequence, 1);

        // Consumer should read the large message
        let read_message = consumer.read().unwrap().unwrap();
        assert_eq!(read_message.data, large_data);

        // Memory stats should reflect the large message
        let memory_stats = manager.memory_stats().unwrap();
        assert!(
            memory_stats.total_bytes > 1024 * 1024,
            "Memory usage should reflect large message"
        );

        println!("✓ Extremely large messages handled correctly");
        println!("  Message size: {} bytes", large_data.len());
        println!("  Total memory usage: {} bytes", memory_stats.total_bytes);
    }

    #[test]
    fn test_many_small_messages_flood() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("flood-producer".to_string())
            .unwrap();
        let consumer = manager
            .create_consumer("flood-consumer".to_string())
            .unwrap();

        // Flood the queue with many small messages
        let message_count = 10000;
        for i in 0..message_count {
            let msg = Message::new(
                "flood-producer".to_string(),
                "flood".to_string(),
                format!("msg-{}", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Verify all messages can be read
        let mut read_count = 0;
        while let Ok(Some(_)) = consumer.read() {
            read_count += 1;
        }

        assert_eq!(
            read_count, message_count,
            "Should read all flooded messages"
        );

        let memory_stats = manager.memory_stats().unwrap();
        println!("✓ Message flood handled correctly");
        println!(
            "  Messages: {}, Memory: {} bytes",
            message_count, memory_stats.total_bytes
        );
    }

    #[test]
    fn test_consumer_drop_edge_cases() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("drop-test-producer".to_string())
            .unwrap();

        // Publish messages
        for i in 0..5 {
            let msg = Message::new(
                "drop-test-producer".to_string(),
                "drop_test".to_string(),
                format!("drop-msg-{}", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Create consumer and read some messages, then drop it
        let initial_consumer_count = manager.active_consumer_count().unwrap();

        {
            let consumer = manager
                .create_consumer("temp-consumer".to_string())
                .unwrap();
            let after_create_count = manager.active_consumer_count().unwrap();
            assert_eq!(after_create_count, initial_consumer_count + 1);

            // Read some messages
            for _ in 0..3 {
                consumer.read().unwrap();
            }

            // Consumer drops here when it goes out of scope
        }

        // Consumer should be automatically unregistered
        let final_consumer_count = manager.active_consumer_count().unwrap();
        assert_eq!(
            final_consumer_count, initial_consumer_count,
            "Consumer should be unregistered on drop"
        );

        // Create a new consumer and verify it doesn't read historical messages
        let new_consumer = manager.create_consumer("new-consumer".to_string()).unwrap();
        let mut remaining_messages = 0;
        while let Ok(Some(_)) = new_consumer.read() {
            remaining_messages += 1;
        }

        // New consumers start from current head, so they won't read historical messages
        assert_eq!(
            remaining_messages, 0,
            "New consumer should not read historical messages"
        );

        println!("✓ Consumer drop edge cases handled correctly");
        println!("  Final consumer count: {}", final_consumer_count);
    }

    #[test]
    fn test_zero_and_negative_batch_sizes() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("batch-edge-producer".to_string())
            .unwrap();
        let consumer = manager
            .create_consumer("batch-edge-consumer".to_string())
            .unwrap();

        // Publish some messages
        for i in 0..5 {
            let msg = Message::new(
                "batch-edge-producer".to_string(),
                "batch_edge".to_string(),
                format!("batch-edge-{}", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Test zero batch size
        let zero_batch = consumer.read_batch(0).unwrap();
        assert!(
            zero_batch.is_empty(),
            "Zero batch size should return empty vector"
        );

        // Test very large batch size (should return all available)
        let large_batch = consumer.read_batch(10000).unwrap();
        assert_eq!(
            large_batch.len(),
            5,
            "Large batch size should return all available messages"
        );

        // Test batch size of 1
        let publisher2 = manager
            .create_publisher("batch-edge-producer-2".to_string())
            .unwrap();
        let consumer2 = manager
            .create_consumer("batch-edge-consumer-2".to_string())
            .unwrap();

        publisher2
            .publish(Message::new(
                "batch-edge-producer-2".to_string(),
                "single".to_string(),
                "single-message".to_string(),
            ))
            .unwrap();

        let single_batch = consumer2.read_batch(1).unwrap();
        assert_eq!(
            single_batch.len(),
            1,
            "Batch size 1 should return single message"
        );

        println!("✓ Batch size edge cases handled correctly");
    }

    #[test]
    fn test_unicode_and_special_characters() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("unicode-producer-🚀".to_string())
            .unwrap();
        let consumer = manager
            .create_consumer("unicode-consumer-🎯".to_string())
            .unwrap();

        // Test various Unicode and special characters
        let test_cases = vec![
            "Hello, 世界! 🌍",
            "Ñoño café résumé naïve façade",
            "Ελληνικά αλφάβητα",
            "Русский текст",
            "🚀🎯🔥💻🌟",
            "\"Quoted string\" with 'nested quotes'",
            "JSON: {\"key\": \"value\", \"number\": 42}",
            "XML: <root><child attr=\"value\">content</child></root>",
            "Special chars: @#$%^&*()_+-=[]{}|;':\",./<>?",
            "\t\n\r\u{0000}\u{001F}", // Control characters
        ];

        // Publish all test cases
        for (_i, test_data) in test_cases.iter().enumerate() {
            let msg = Message::new(
                format!("unicode-producer-🚀"),
                "unicode_test".to_string(),
                test_data.to_string(),
            );
            publisher.publish(msg).unwrap();
        }

        // Read and verify all messages
        for (i, expected_data) in test_cases.iter().enumerate() {
            let message = consumer.read().unwrap().unwrap();
            assert_eq!(
                &message.data, expected_data,
                "Unicode message {} should match: expected '{}', got '{}'",
                i, expected_data, message.data
            );
            assert_eq!(message.header.producer_id, "unicode-producer-🚀");
        }

        println!("✓ Unicode and special character handling verified");
        println!(
            "  Processed {} diverse character test cases",
            test_cases.len()
        );
    }

    #[tokio::test]
    async fn test_concurrent_consumer_cleanup_race_conditions() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("race-producer".to_string())
            .unwrap();

        // Publish messages
        for i in 0..100 {
            let msg = Message::new(
                "race-producer".to_string(),
                "race_test".to_string(),
                format!("race-msg-{}", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Create many consumers concurrently
        let mut handles = Vec::new();
        for consumer_id in 0..20 {
            let manager_clone = Arc::clone(&manager);

            let handle = tokio::spawn(async move {
                let consumer = manager_clone
                    .create_consumer(format!("race-consumer-{}", consumer_id))
                    .unwrap();

                // Each consumer reads some messages then drops
                let mut read_count = 0;
                for _ in 0..5 {
                    if let Ok(Some(_)) = consumer.read() {
                        read_count += 1;
                    }
                    tokio::time::sleep(Duration::from_micros(10)).await;
                }

                // Consumer drops when function ends
                read_count
            });

            handles.push(handle);
        }

        // Wait for all consumers to complete
        let mut total_reads = 0;
        for handle in handles {
            let reads = handle.await.unwrap();
            total_reads += reads;
        }

        // Verify system is still functional after concurrent cleanup
        let final_consumer_count = manager.active_consumer_count().unwrap();
        assert_eq!(
            final_consumer_count, 0,
            "All consumers should be cleaned up"
        );

        let memory_stats = manager.memory_stats().unwrap();
        assert!(
            memory_stats.total_bytes > 0,
            "Should still have messages in queue"
        );

        println!("✓ Concurrent consumer cleanup race conditions handled");
        println!("  Total reads by all consumers: {}", total_reads);
        println!("  Final consumer count: {}", final_consumer_count);
    }

    #[test]
    fn test_garbage_collection_edge_cases() {
        let manager = Arc::new(QueueManager::new());
        let publisher = manager
            .create_publisher("gc-edge-producer".to_string())
            .unwrap();

        // Test garbage collection on empty queue
        let collected_empty = manager.collect_garbage().unwrap();
        assert_eq!(
            collected_empty, 0,
            "Garbage collection on empty queue should collect 0"
        );

        // Publish messages
        for i in 0..10 {
            let msg = Message::new(
                "gc-edge-producer".to_string(),
                "gc_test".to_string(),
                format!("gc-msg-{}", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Test garbage collection with no consumers (should not collect anything)
        let collected_no_consumers = manager.collect_garbage().unwrap();
        assert_eq!(
            collected_no_consumers, 0,
            "GC with no consumers should collect nothing"
        );

        // Create consumer and read some messages
        let consumer = manager.create_consumer("gc-consumer".to_string()).unwrap();
        for _ in 0..5 {
            consumer.read().unwrap();
        }

        // Now garbage collection should clean up read messages
        let collected_with_consumer = manager.collect_garbage().unwrap();
        assert!(
            collected_with_consumer > 0,
            "GC should collect read messages"
        );

        // Drop consumer and test orphaned consumer cleanup
        drop(consumer);

        // Force another garbage collection
        let collected_after_drop = manager.collect_garbage().unwrap();
        // May or may not collect more depending on implementation

        println!("✓ Garbage collection edge cases handled correctly");
        println!(
            "  Empty queue: {}, No consumers: {}, With consumer: {}, After drop: {}",
            collected_empty, collected_no_consumers, collected_with_consumer, collected_after_drop
        );
    }
}
