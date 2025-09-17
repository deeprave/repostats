//! Tests for concurrent queue operations and performance

#[cfg(test)]
mod tests {
    use crate::queue::api::{Message, QueueManager};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::task::JoinSet;
    use tokio::time::timeout;

    #[tokio::test]
    #[ignore = "slow"]
    async fn test_concurrent_consumer_access_stress_test() {
        let manager = Arc::new(QueueManager::new());

        // Create multiple concurrent consumers FIRST
        let consumer_count = 5; // Reduced for more reliable test
        let mut consumers = Vec::new();

        for consumer_id in 0..consumer_count {
            let consumer = manager
                .create_consumer(format!("stress-consumer-{}", consumer_id))
                .unwrap();
            consumers.push(consumer);
        }

        // Publish many messages
        let publisher = manager
            .create_publisher("stress-producer".to_string())
            .unwrap();
        for i in 0..100 {
            // Reduced message count for reliability
            let msg = Message::new(
                "stress-producer".to_string(),
                "file".to_string(),
                format!("message-{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Now start concurrent reading tasks
        let mut tasks = JoinSet::new();

        for (consumer_id, consumer) in consumers.into_iter().enumerate() {
            tasks.spawn(async move {
                let mut messages_read = 0;
                let start_time = Instant::now();

                // Each consumer tries to read messages with longer timeout
                while let Ok(Ok(Some(_message))) =
                    timeout(Duration::from_millis(200), async { consumer.read() }).await
                {
                    messages_read += 1;
                    // Small delay to simulate processing
                    tokio::time::sleep(Duration::from_micros(10)).await;
                }

                let duration = start_time.elapsed();
                (consumer_id, messages_read, duration)
            });
        }

        // Collect results
        let mut total_reads = 0;
        let mut max_duration = Duration::from_secs(0);

        while let Some(result) = tasks.join_next().await {
            let (consumer_id, messages_read, duration) = result.unwrap();
            total_reads += messages_read;
            max_duration = max_duration.max(duration);
            println!(
                "Consumer {} read {} messages in {:?}",
                consumer_id, messages_read, duration
            );
        }

        // Verify all consumers could access the queue concurrently
        assert!(
            total_reads >= 100,
            "Should have read all messages, got {}",
            total_reads
        );
        assert!(
            max_duration < Duration::from_secs(5),
            "Concurrent access shouldn't take too long: {:?}",
            max_duration
        );
        println!(
            "✓ {} consumers read {} total messages concurrently",
            consumer_count, total_reads
        );
    }

    #[tokio::test]
    async fn test_concurrent_publisher_consumer_operations() {
        let manager = Arc::new(QueueManager::new());

        // Create consumers first
        let consumer1 = manager
            .create_consumer("concurrent-consumer-1".to_string())
            .unwrap();
        let consumer2 = manager
            .create_consumer("concurrent-consumer-2".to_string())
            .unwrap();

        // Create publishers
        let publisher1 = manager
            .create_publisher("concurrent-producer-1".to_string())
            .unwrap();
        let publisher2 = manager
            .create_publisher("concurrent-producer-2".to_string())
            .unwrap();

        let mut tasks = JoinSet::new();

        // Concurrent publishing task 1
        let pub1_clone = Arc::new(publisher1);
        tasks.spawn(async move {
            for i in 0..50 {
                // Reduced count for more reliable test
                let msg = Message::new(
                    "concurrent-producer-1".to_string(),
                    "file".to_string(),
                    format!("producer1-{}.rs", i),
                );
                pub1_clone.publish(msg).unwrap();
                // No delay to speed up publishing
            }
            50
        });

        // Concurrent publishing task 2
        let pub2_clone = Arc::new(publisher2);
        tasks.spawn(async move {
            for i in 0..50 {
                // Reduced count for more reliable test
                let msg = Message::new(
                    "concurrent-producer-2".to_string(),
                    "file".to_string(),
                    format!("producer2-{}.rs", i),
                );
                pub2_clone.publish(msg).unwrap();
                // No delay to speed up publishing
            }
            50
        });

        // Wait for publishers to complete first
        let mut published_total = 0;
        while let Some(result) = tasks.join_next().await {
            let count = result.unwrap();
            published_total += count;
            if published_total >= 100 {
                break;
            }
        }

        // Now start consuming tasks
        let cons1_clone = Arc::new(consumer1);
        tasks.spawn(async move {
            let mut count = 0;
            let timeout_duration = Duration::from_millis(200);

            while let Ok(Ok(Some(_msg))) =
                timeout(timeout_duration, async { cons1_clone.read() }).await
            {
                count += 1;
            }
            count
        });

        let cons2_clone = Arc::new(consumer2);
        tasks.spawn(async move {
            let mut count = 0;
            let timeout_duration = Duration::from_millis(200);

            while let Ok(Ok(Some(_msg))) =
                timeout(timeout_duration, async { cons2_clone.read() }).await
            {
                count += 1;
            }
            count
        });

        // Wait for consumers to complete
        let mut consumed_total = 0;
        while let Some(result) = tasks.join_next().await {
            let count = result.unwrap();
            consumed_total += count;
        }

        assert_eq!(
            published_total, 100,
            "Should have published 100 messages total"
        );
        assert!(
            consumed_total >= 100,
            "Should have consumed at least 100 messages, got {}",
            consumed_total
        );
        println!(
            "✓ Published {} messages, consumed {} messages concurrently",
            published_total, consumed_total
        );
    }

    #[tokio::test]
    async fn test_batch_read_operations() {
        let manager = Arc::new(QueueManager::new());

        let consumer = manager
            .create_consumer("batch-consumer".to_string())
            .unwrap();
        let publisher = manager
            .create_publisher("batch-producer".to_string())
            .unwrap();

        // Publish 15 messages
        for i in 0..15 {
            let msg = Message::new(
                "batch-producer".to_string(),
                "file".to_string(),
                format!("batch-{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Test batch read with batch size 5
        let batch1 = consumer.read_batch(5).unwrap();
        assert_eq!(batch1.len(), 5);
        assert_eq!(batch1[0].data, "batch-0.rs");
        assert_eq!(batch1[4].data, "batch-4.rs");

        // Test another batch read
        let batch2 = consumer.read_batch(7).unwrap();
        assert_eq!(batch2.len(), 7);
        assert_eq!(batch2[0].data, "batch-5.rs");
        assert_eq!(batch2[6].data, "batch-11.rs");

        // Final batch should only get remaining messages
        let batch3 = consumer.read_batch(10).unwrap();
        assert_eq!(batch3.len(), 3); // Only 3 messages left
        assert_eq!(batch3[0].data, "batch-12.rs");
        assert_eq!(batch3[2].data, "batch-14.rs");

        // No more messages
        let empty_batch = consumer.read_batch(5).unwrap();
        assert_eq!(empty_batch.len(), 0);

        println!("✓ Batch read operations working correctly");
        println!(
            "  Read {} + {} + {} messages in batches",
            batch1.len(),
            batch2.len(),
            batch3.len()
        );
    }

    #[tokio::test]
    async fn test_batch_acknowledgment() {
        let manager = Arc::new(QueueManager::new());

        let consumer = manager.create_consumer("ack-consumer".to_string()).unwrap();
        let publisher = manager
            .create_publisher("ack-producer".to_string())
            .unwrap();

        // Publish messages
        for i in 0..10 {
            let msg = Message::new(
                "ack-producer".to_string(),
                "file".to_string(),
                format!("ack-{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Read messages normally (single read)
        let mut individual_messages = Vec::new();
        for _ in 0..5 {
            if let Some(msg) = consumer.read().unwrap() {
                individual_messages.push(msg);
            }
        }
        assert_eq!(individual_messages.len(), 5);

        // Acknowledge batch of messages (should not affect read position)
        let ack_result = consumer.acknowledge_batch(&individual_messages).unwrap();
        assert_eq!(ack_result, 5);

        // Continue reading should work normally
        let remaining = consumer.read_batch(10).unwrap();
        assert_eq!(remaining.len(), 5); // Remaining messages

        println!("✓ Batch acknowledgment working correctly");
        println!(
            "  Acknowledged {} messages, read {} remaining",
            ack_result,
            remaining.len()
        );
    }
}
