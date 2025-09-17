//! Integration tests for the entire queue system
//!
//! These tests verify that all components work together correctly,
//! simulating real-world usage patterns with multiple producers,
//! consumers, and various operational scenarios.

#[cfg(test)]
mod tests {
    use crate::queue::api::{Message, QueueManager};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn test_full_producer_consumer_workflow() {
        let manager = Arc::new(QueueManager::new());

        // Create multiple producers (simulating different components)
        let file_scanner = manager
            .create_publisher("file-scanner".to_string())
            .unwrap();
        let git_monitor = manager.create_publisher("git-monitor".to_string()).unwrap();
        let config_watcher = manager
            .create_publisher("config-watcher".to_string())
            .unwrap();

        // Create multiple consumers (simulating different plugins)
        let linter_plugin = manager
            .create_consumer("linter-plugin".to_string())
            .unwrap();
        let formatter_plugin = manager
            .create_consumer("formatter-plugin".to_string())
            .unwrap();
        let metrics_plugin = manager
            .create_consumer("metrics-plugin".to_string())
            .unwrap();

        // Phase 1: Producers publish different types of messages
        let mut published_messages = Vec::new();

        // File scanner publishes file discovery messages
        for i in 0..5 {
            let msg = Message::new(
                "file-scanner".to_string(),
                "file_discovered".to_string(),
                format!("src/module_{}.rs", i),
            );
            file_scanner.publish(msg.clone()).unwrap();
            published_messages.push(msg);
        }

        // Git monitor publishes change notifications
        for i in 0..3 {
            let msg = Message::new(
                "git-monitor".to_string(),
                "file_changed".to_string(),
                format!("src/changed_{}.rs", i),
            );
            git_monitor.publish(msg.clone()).unwrap();
            published_messages.push(msg);
        }

        // Config watcher publishes configuration updates
        for i in 0..2 {
            let msg = Message::new(
                "config-watcher".to_string(),
                "config_updated".to_string(),
                format!("config_{}.toml", i),
            );
            config_watcher.publish(msg.clone()).unwrap();
            published_messages.push(msg);
        }

        // Phase 2: Consumers process messages independently
        let mut consumer_results = Vec::new();

        // Each consumer reads all available messages
        let consumers = vec![
            ("linter", linter_plugin),
            ("formatter", formatter_plugin),
            ("metrics", metrics_plugin),
        ];

        for (name, consumer) in consumers {
            let mut messages_read = Vec::new();

            // Read all available messages
            while let Ok(Some(message)) = consumer.read() {
                messages_read.push(message);
            }

            consumer_results.push((name, messages_read));
        }

        // Phase 3: Verify integration correctness
        let total_published = published_messages.len();
        assert_eq!(
            total_published, 10,
            "Should have published 10 messages total"
        );

        // Each consumer should have read all messages
        for (consumer_name, messages_read) in &consumer_results {
            assert_eq!(
                messages_read.len(),
                total_published,
                "Consumer '{}' should have read all {} messages, but read {}",
                consumer_name,
                total_published,
                messages_read.len()
            );
        }

        // Verify messages are in correct sequence order
        for (consumer_name, messages_read) in &consumer_results {
            for (i, message) in messages_read.iter().enumerate() {
                let expected_message = &published_messages[i];
                assert_eq!(
                    message.data, expected_message.data,
                    "Consumer '{}' message {} data mismatch",
                    consumer_name, i
                );
                assert_eq!(
                    message.header.producer_id, expected_message.header.producer_id,
                    "Consumer '{}' message {} producer_id mismatch",
                    consumer_name, i
                );
            }
        }

        println!("✓ Full producer-consumer workflow integration test passed");
        println!("  Published: {} messages from 3 producers", total_published);
        println!(
            "  Consumed: {} messages by 3 consumers",
            total_published * 3
        );
    }

    #[tokio::test]
    async fn test_concurrent_integration_basic() {
        let manager = Arc::new(QueueManager::new());

        let mut tasks = JoinSet::new();

        // Spawn concurrent producer tasks
        for producer_id in 0..3 {
            let manager_clone = Arc::clone(&manager);

            tasks.spawn(async move {
                let producer = manager_clone
                    .create_publisher(format!("concurrent-producer-{}", producer_id))
                    .unwrap();

                let mut published = 0;

                for i in 0..20 {
                    let msg = Message::new(
                        format!("concurrent-producer-{}", producer_id),
                        "data".to_string(),
                        format!("message-{}-{}", producer_id, i),
                    );

                    match producer.publish(msg) {
                        Ok(_) => published += 1,
                        Err(e) => {
                            println!(
                                "Producer {} failed to publish message {}: {:?}",
                                producer_id, i, e
                            );
                            break;
                        }
                    }

                    // Small delay to simulate realistic publishing rate
                    tokio::time::sleep(Duration::from_micros(100)).await;
                }

                (producer_id, published)
            });
        }

        // Spawn concurrent consumer tasks
        for consumer_id in 0..2 {
            let manager_clone = Arc::clone(&manager);

            tasks.spawn(async move {
                let consumer = manager_clone
                    .create_consumer(format!("concurrent-consumer-{}", consumer_id))
                    .unwrap();

                let mut consumed = 0;
                let start_time = std::time::Instant::now();

                // Consumer reads with timeout to avoid hanging
                while start_time.elapsed() < Duration::from_secs(3) {
                    match consumer.read() {
                        Ok(Some(_message)) => {
                            consumed += 1;
                            // Simulate processing time
                            tokio::time::sleep(Duration::from_micros(50)).await;
                        }
                        Ok(None) => {
                            // No more messages, brief pause before retry
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        Err(e) => {
                            println!("Consumer {} error: {:?}", consumer_id, e);
                            break;
                        }
                    }
                }

                (consumer_id, consumed)
            });
        }

        // Wait for all tasks to complete
        let mut producer_totals = 0;
        let mut consumer_totals = 0;

        while let Some(result) = tasks.join_next().await {
            let (id, count) = result.unwrap();

            // Distinguish between producer and consumer results
            if count > 30 {
                // Likely a consumer (reads more than publishes due to sharing)
                consumer_totals += count;
                println!("Consumer {} processed {} messages", id, count);
            } else {
                // Likely a producer
                producer_totals += count;
                println!("Producer {} published {} messages", id, count);
            }
        }

        // Verify integration under concurrent load
        assert!(producer_totals > 0, "Should have published some messages");
        assert!(consumer_totals > 0, "Should have consumed some messages");

        println!("✓ Concurrent integration test completed");
        println!(
            "  Total published: {}, Total consumed: {}",
            producer_totals, consumer_totals
        );
    }

    #[tokio::test]
    async fn test_lifecycle_integration_with_events() {
        // This test verifies the integration between queue lifecycle and event system
        let manager = QueueManager::create().await;

        // Create some activity
        let publisher = manager
            .create_publisher("lifecycle-producer".to_string())
            .unwrap();
        let consumer = manager
            .create_consumer("lifecycle-consumer".to_string())
            .unwrap();

        // Publish and consume some messages
        for i in 0..5 {
            let msg = Message::new(
                "lifecycle-producer".to_string(),
                "lifecycle_event".to_string(),
                format!("lifecycle-message-{}.txt", i),
            );
            publisher.publish(msg).unwrap();
        }

        let mut consumed = 0;
        while let Ok(Some(_)) = consumer.read() {
            consumed += 1;
        }

        assert_eq!(consumed, 5, "Should consume all lifecycle messages");

        // Verify the queue manager is functional after event integration
        let active_consumers = manager.active_consumer_count().unwrap();
        assert_eq!(active_consumers, 1, "Should have one active consumer");

        println!("✓ Lifecycle integration with events test passed");
        println!("  Processed: {} lifecycle messages", consumed);
        println!("  Active consumers: {}", active_consumers);
    }
}
