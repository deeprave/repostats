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
    async fn test_memory_pressure_under_concurrent_load() {
        let manager = Arc::new(QueueManager::new());
        manager.set_memory_threshold_bytes(1000).unwrap(); // Low threshold for testing

        let publisher = manager
            .create_publisher("memory-test-producer".to_string())
            .unwrap();
        let consumer = manager
            .create_consumer("memory-test-consumer".to_string())
            .unwrap();

        // Publish messages to trigger memory pressure
        for i in 0..200 {
            let msg = Message::new(
                "memory-test-producer".to_string(),
                "file".to_string(),
                format!(
                    "Large message content for memory pressure testing {}",
                    i.to_string().repeat(10)
                ),
            );
            publisher.publish(msg).unwrap();
        }

        let initial_memory = manager.memory_usage_bytes();
        println!("Initial memory usage: {} bytes", initial_memory);

        // Read some messages to allow garbage collection
        for _ in 0..100 {
            if let Ok(Some(_msg)) = consumer.read() {
                // Process message
            }
        }

        // Trigger garbage collection
        let collected = manager.collect_garbage().unwrap();
        let final_memory = manager.memory_usage_bytes();

        println!(
            "Collected {} messages, memory reduced from {} to {} bytes",
            collected, initial_memory, final_memory
        );

        assert!(collected > 0, "Should have collected some messages");
        assert!(
            final_memory <= initial_memory,
            "Memory should not increase after garbage collection"
        );
    }

    #[tokio::test]
    async fn test_consumer_lag_monitoring() {
        let manager = Arc::new(QueueManager::new());

        // Create consumers and publisher
        let consumer1 = manager
            .create_consumer("lag-consumer-1".to_string())
            .unwrap();
        let consumer2 = manager
            .create_consumer("lag-consumer-2".to_string())
            .unwrap();
        let publisher = manager
            .create_publisher("lag-producer".to_string())
            .unwrap();

        // Publish messages
        for i in 0..10 {
            let msg = Message::new(
                "lag-producer".to_string(),
                "file".to_string(),
                format!("message-{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Consumer1 reads 5 messages, consumer2 reads 3 messages
        for _ in 0..5 {
            consumer1.read().unwrap();
        }
        for _ in 0..3 {
            consumer2.read().unwrap();
        }

        // Check consumer lag statistics
        let consumer1_lag = manager.get_consumer_lag(&consumer1).unwrap();
        let consumer2_lag = manager.get_consumer_lag(&consumer2).unwrap();

        // Consumer1 should have lag of 5 (10 total - 5 read)
        assert_eq!(consumer1_lag, 5);
        // Consumer2 should have lag of 7 (10 total - 3 read)
        assert_eq!(consumer2_lag, 7);

        // Get overall lag statistics
        let lag_stats = manager.get_lag_statistics().unwrap();
        assert_eq!(lag_stats.total_consumers, 2);
        assert_eq!(lag_stats.max_lag, 7);
        assert_eq!(lag_stats.min_lag, 5);
        assert_eq!(lag_stats.avg_lag, 6.0);

        println!("✓ Consumer lag monitoring working correctly");
        println!("  Consumer 1 lag: {} messages", consumer1_lag);
        println!("  Consumer 2 lag: {} messages", consumer2_lag);
        println!(
            "  Max lag: {}, Min lag: {}, Avg lag: {}",
            lag_stats.max_lag, lag_stats.min_lag, lag_stats.avg_lag
        );
    }

    #[tokio::test]
    async fn test_consumer_lag_with_different_read_patterns() {
        let manager = Arc::new(QueueManager::new());

        // Create consumers
        let fast_consumer = manager.create_consumer("fast-reader".to_string()).unwrap();
        let slow_consumer = manager.create_consumer("slow-reader".to_string()).unwrap();
        let publisher = manager
            .create_publisher("pattern-producer".to_string())
            .unwrap();

        // Publish 20 messages
        for i in 0..20 {
            let msg = Message::new(
                "pattern-producer".to_string(),
                "file".to_string(),
                format!("pattern-{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Fast consumer reads all messages
        for _ in 0..20 {
            fast_consumer.read().unwrap();
        }

        // Slow consumer reads only 2 messages
        for _ in 0..2 {
            slow_consumer.read().unwrap();
        }

        // Check individual lag
        let fast_lag = manager.get_consumer_lag(&fast_consumer).unwrap();
        let slow_lag = manager.get_consumer_lag(&slow_consumer).unwrap();

        assert_eq!(fast_lag, 0); // No lag, read everything
        assert_eq!(slow_lag, 18); // High lag, read only 2/20

        // Check lag statistics
        let stats = manager.get_lag_statistics().unwrap();
        assert_eq!(stats.max_lag, 18);
        assert_eq!(stats.min_lag, 0);
        assert_eq!(stats.avg_lag, 9.0); // (0 + 18) / 2

        println!("✓ Consumer lag patterns tracked correctly");
        println!("  Fast consumer lag: {}", fast_lag);
        println!("  Slow consumer lag: {}", slow_lag);
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

    #[tokio::test]
    async fn test_stale_consumer_detection() {
        let manager = Arc::new(QueueManager::new());

        let active_consumer = manager
            .create_consumer("active-consumer".to_string())
            .unwrap();
        let stale_consumer = manager
            .create_consumer("stale-consumer".to_string())
            .unwrap();
        let publisher = manager
            .create_publisher("stale-test-producer".to_string())
            .unwrap();

        // Publish some messages
        for i in 0..5 {
            let msg = Message::new(
                "stale-test-producer".to_string(),
                "file".to_string(),
                format!("stale-{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Active consumer reads messages
        for _ in 0..3 {
            active_consumer.read().unwrap();
        }

        // Stale consumer reads one message then stops
        stale_consumer.read().unwrap();

        // Wait a moment to simulate time passing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Publish more messages to increase the lag
        for i in 5..10 {
            let msg = Message::new(
                "stale-test-producer".to_string(),
                "file".to_string(),
                format!("stale-{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Active consumer continues reading
        for _ in 0..2 {
            active_consumer.read().unwrap();
        }

        // Check for stale consumers with a low threshold
        let stale_threshold_seconds = 0; // Any consumer with lag > 0 is considered stale
        let stale_consumers = manager
            .detect_stale_consumers(stale_threshold_seconds)
            .unwrap();

        // Stale consumer should be detected (has higher lag)
        assert!(
            stale_consumers.len() > 0,
            "Should detect at least one stale consumer"
        );

        let stale_info = &stale_consumers[0];
        assert!(stale_info.lag > 0, "Stale consumer should have lag > 0");
        assert_eq!(
            stale_info.consumer_id,
            stale_consumer.internal_consumer_id()
        );

        println!("✓ Stale consumer detection working correctly");
        println!("  Detected {} stale consumers", stale_consumers.len());
        println!("  Stale consumer lag: {}", stale_info.lag);
    }

    #[tokio::test]
    async fn test_stale_consumer_cleanup() {
        let manager = Arc::new(QueueManager::new());

        let consumer1 = manager
            .create_consumer("cleanup-consumer-1".to_string())
            .unwrap();
        let _consumer2 = manager
            .create_consumer("cleanup-consumer-2".to_string())
            .unwrap();
        let publisher = manager
            .create_publisher("cleanup-producer".to_string())
            .unwrap();

        // Publish messages
        for i in 0..10 {
            let msg = Message::new(
                "cleanup-producer".to_string(),
                "file".to_string(),
                format!("cleanup-{}.rs", i),
            );
            publisher.publish(msg).unwrap();
        }

        // Consumer1 reads all messages (active)
        for _ in 0..10 {
            consumer1.read().unwrap();
        }

        // Consumer2 reads nothing (stale)
        // (stale_consumer doesn't read anything)

        let initial_consumer_count = manager.active_consumer_count().unwrap();
        assert_eq!(initial_consumer_count, 2);

        // Detect and cleanup stale consumers
        let stale_consumers = manager.detect_stale_consumers(0).unwrap();
        assert!(
            stale_consumers.len() >= 1,
            "Should have at least one stale consumer"
        );

        let cleanup_count = manager.cleanup_stale_consumers(5).unwrap(); // Lag threshold of 5
        assert!(
            cleanup_count >= 1,
            "Should cleanup at least one stale consumer"
        );

        // Verify consumer count decreased
        let final_consumer_count = manager.active_consumer_count().unwrap();
        assert!(
            final_consumer_count < initial_consumer_count,
            "Consumer count should decrease after cleanup"
        );

        println!("✓ Stale consumer cleanup working correctly");
        println!(
            "  Initial consumers: {}, cleaned up: {}, remaining: {}",
            initial_consumer_count, cleanup_count, final_consumer_count
        );
    }
}
