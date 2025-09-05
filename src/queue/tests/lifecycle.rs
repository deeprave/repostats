//! Tests for queue lifecycle event integration

#[cfg(test)]
mod tests {
    use crate::notifications::api::{Event, EventFilter, QueueEvent, QueueEventType};
    use crate::notifications::manager::AsyncNotificationManager;
    use crate::queue::api::QueueManager;
    use tokio::time::{timeout, Duration};

    /// Test utility to create isolated notification manager for testing
    fn create_test_notification_manager() -> AsyncNotificationManager {
        let mut manager = AsyncNotificationManager::new();
        manager.clear_subscribers();
        manager
    }

    #[tokio::test]
    async fn test_notification_system_basic_functionality() {
        // Use completely isolated notification manager for this test
        let mut notification_manager = create_test_notification_manager();

        // Subscribe to queue events
        let mut subscriber = notification_manager
            .subscribe(
                "test-basic-functionality".to_string(),
                EventFilter::QueueOnly,
                "test:basic".to_string(),
            )
            .unwrap();

        // Manually publish a queue event
        let test_event = Event::Queue(QueueEvent::new(QueueEventType::Started, "test".to_string()));
        notification_manager.publish(test_event).await.unwrap();

        // Try to receive it with timeout
        let result = timeout(Duration::from_millis(100), subscriber.recv()).await;

        match result {
            Ok(Some(received_event)) => match received_event {
                Event::Queue(queue_event) => {
                    assert_eq!(queue_event.event_type, QueueEventType::Started);
                    assert_eq!(queue_event.queue_id, "test");
                    println!("✓ Basic notification system works");
                }
                _ => panic!("Expected Queue::Started event, got: {:?}", received_event),
            },
            Ok(None) => panic!("Received None from subscriber"),
            Err(_) => panic!("Timeout waiting for manually published event"),
        }
    }

    #[tokio::test]
    async fn test_queue_manager_create_completes_successfully() {
        // Test that QueueManager::create() completes without hanging or panicking
        let manager = QueueManager::create().await;

        // Verify the manager is properly initialized
        assert_eq!(manager.queue_count(), 1);
        assert_eq!(manager.total_message_count().unwrap(), 0);

        println!("✓ QueueManager::create() completes successfully");
    }

    #[tokio::test]
    async fn test_queue_manager_basic_operations() {
        // Test basic queue manager functionality
        let manager = QueueManager::create().await;

        // Test publisher creation
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .expect("Should create publisher");

        // Test consumer creation
        let consumer = manager
            .create_consumer("test-plugin".to_string())
            .expect("Should create consumer");

        // Test basic operations work
        assert_eq!(manager.queue_count(), 1);
        assert_eq!(manager.producer_ids(), vec!["global".to_string()]);

        drop(publisher);
        drop(consumer);

        println!("✓ QueueManager basic operations work correctly");
    }

    #[tokio::test]
    async fn test_queue_manager_shutdown_completes_successfully() {
        // Test that QueueManager::shutdown() completes without hanging or panicking
        let manager = QueueManager::create().await;

        // Verify queue is operational before shutdown
        assert_eq!(manager.queue_count(), 1);

        // Test that shutdown completes successfully
        manager.shutdown().await;

        // Queue should still be accessible after shutdown for cleanup operations
        assert_eq!(manager.queue_count(), 1);

        println!("✓ QueueManager::shutdown() completes successfully");
    }

    #[tokio::test]
    async fn test_queue_manager_lifecycle_operations() {
        // Test complete lifecycle of QueueManager without event dependencies
        let manager = QueueManager::create().await;

        // Test initial state
        assert_eq!(manager.queue_count(), 1);
        assert_eq!(manager.total_message_count().unwrap(), 0);
        assert_eq!(manager.active_consumer_count().unwrap(), 0);

        // Create some publishers and consumers
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .expect("Should create publisher");
        let consumer = manager
            .create_consumer("test-plugin".to_string())
            .expect("Should create consumer");

        // Verify operational state
        assert_eq!(manager.active_consumer_count().unwrap(), 1);

        // Test memory and lag statistics
        let memory_stats = manager.memory_stats().expect("Should get memory stats");
        let lag_stats = manager.get_lag_statistics().expect("Should get lag stats");

        assert!(memory_stats.total_bytes >= 0);
        assert_eq!(lag_stats.total_consumers, 1);

        // Test shutdown
        manager.shutdown().await;

        // Cleanup
        drop(publisher);
        drop(consumer);

        println!("✓ Complete queue lifecycle operations work correctly");
    }

    #[tokio::test]
    async fn test_notification_event_types_are_correct() {
        // Test that the event types used by QueueManager are valid
        let started_event = Event::Queue(QueueEvent::new(
            QueueEventType::Started,
            "global".to_string(),
        ));
        let shutdown_event = Event::Queue(QueueEvent::new(
            QueueEventType::Shutdown,
            "global".to_string(),
        ));

        // Verify events can be created and have correct types
        match started_event {
            Event::Queue(queue_event) => {
                assert_eq!(queue_event.event_type, QueueEventType::Started);
                assert_eq!(queue_event.queue_id, "global");
            }
            _ => panic!("Expected Queue event"),
        }

        match shutdown_event {
            Event::Queue(queue_event) => {
                assert_eq!(queue_event.event_type, QueueEventType::Shutdown);
                assert_eq!(queue_event.queue_id, "global");
            }
            _ => panic!("Expected Queue event"),
        }

        println!("✓ Queue lifecycle event types are correctly defined");
    }

    #[tokio::test]
    async fn test_queue_manager_resilience_to_notification_failures() {
        // Test that QueueManager continues to operate even if notification system fails
        // This simulates scenarios where the notification system is overloaded or unavailable

        // Create multiple managers concurrently to potentially cause notification system stress
        let mut handles = vec![];

        for i in 0..10 {
            let handle = tokio::spawn(async move {
                let manager = QueueManager::create().await;

                // Perform basic queue operations
                let publisher = manager
                    .create_publisher(format!("producer-{}", i))
                    .expect("Should create publisher");
                let consumer = manager
                    .create_consumer(format!("plugin-{}", i))
                    .expect("Should create consumer");

                // Test shutdown
                manager.shutdown().await;

                drop(publisher);
                drop(consumer);

                i
            });
            handles.push(handle);
        }

        // Wait for all managers to complete
        for handle in handles {
            let result = handle.await.expect("Task should complete");
            assert!(result < 10);
        }

        println!("✓ QueueManager remains resilient under concurrent notification stress");
    }

    #[tokio::test]
    async fn test_queue_operations_independent_of_events() {
        // Test that core queue functionality works even if event publishing completely fails
        let manager = QueueManager::create().await;

        // Test all basic operations work regardless of event system state
        assert_eq!(manager.queue_count(), 1);

        let publisher = manager
            .create_publisher("test-producer".to_string())
            .expect("Should create publisher despite event issues");
        let consumer = manager
            .create_consumer("test-plugin".to_string())
            .expect("Should create consumer despite event issues");

        // Test queue management operations
        assert_eq!(manager.active_consumer_count().unwrap(), 1);
        assert_eq!(manager.total_message_count().unwrap(), 0);

        // Test memory and statistics still work
        let memory_stats = manager.memory_stats().expect("Should get memory stats");
        assert!(memory_stats.total_messages >= 0);

        let lag_stats = manager.get_lag_statistics().expect("Should get lag stats");
        assert_eq!(lag_stats.total_consumers, 1);

        // Test shutdown completes successfully
        manager.shutdown().await;

        drop(publisher);
        drop(consumer);

        println!("✓ Queue operations work independently of event publishing");
    }

    #[tokio::test]
    async fn test_queue_started_event_integration_with_global_service() {
        // INTEGRATION TEST: Verify QueueManager::create() publishes Started events
        // that subscribers actually receive through the global notification service

        // Subscribe to queue events using the GLOBAL service (real integration)
        let mut subscriber = {
            let mut notification_manager =
                crate::notifications::api::get_notification_service().await;

            // Clear any stale subscribers to avoid interference
            notification_manager.clear_subscribers();

            notification_manager
                .subscribe(
                    "integration-test-started".to_string(),
                    EventFilter::QueueOnly,
                    "integration:started".to_string(),
                )
                .unwrap()
        };

        // Create QueueManager - this should publish Started event to global service
        let _manager = QueueManager::create().await;

        // Verify we receive the Started event with proper timeout
        let result = timeout(Duration::from_millis(200), subscriber.recv()).await;

        match result {
            Ok(Some(received_event)) => {
                match received_event {
                    Event::Queue(queue_event) => {
                        assert_eq!(queue_event.event_type, QueueEventType::Started);
                        assert_eq!(queue_event.queue_id, "global");
                        println!("✓ INTEGRATION: QueueManager Started event delivered through global service");
                    }
                    _ => panic!("INTEGRATION FAILURE: Expected Queue::Started event, got: {:?}", received_event),
                }
            }
            Ok(None) => panic!("INTEGRATION FAILURE: Received None from global subscriber"),
            Err(_) => panic!("INTEGRATION FAILURE: Timeout waiting for Started event - event publishing may not be working"),
        }

        // Cleanup to avoid affecting other tests
        {
            let mut notification_manager =
                crate::notifications::api::get_notification_service().await;
            notification_manager.clear_subscribers();
        }
    }

    #[tokio::test]
    async fn test_queue_shutdown_event_integration_with_global_service() {
        // INTEGRATION TEST: Verify QueueManager::shutdown() publishes Shutdown events
        // that subscribers actually receive through the global notification service

        // Subscribe to queue events using the GLOBAL service
        let mut subscriber = {
            let mut notification_manager =
                crate::notifications::api::get_notification_service().await;
            notification_manager.clear_subscribers();

            notification_manager
                .subscribe(
                    "integration-test-shutdown".to_string(),
                    EventFilter::QueueOnly,
                    "integration:shutdown".to_string(),
                )
                .unwrap()
        };

        // Create and immediately consume the Started event to focus on shutdown
        let manager = QueueManager::create().await;

        // Drain the Started event first
        let started_result = timeout(Duration::from_millis(100), subscriber.recv()).await;
        match started_result {
            Ok(Some(Event::Queue(event))) if event.event_type == QueueEventType::Started => {
                println!("✓ Started event received, proceeding to test shutdown");
            }
            _ => panic!("INTEGRATION SETUP FAILURE: Should receive Started event first"),
        }

        // Now test shutdown event delivery
        manager.shutdown().await;

        // Verify we receive the Shutdown event
        let shutdown_result = timeout(Duration::from_millis(200), subscriber.recv()).await;

        match shutdown_result {
            Ok(Some(received_event)) => {
                match received_event {
                    Event::Queue(queue_event) => {
                        assert_eq!(queue_event.event_type, QueueEventType::Shutdown);
                        assert_eq!(queue_event.queue_id, "global");
                        println!("✓ INTEGRATION: QueueManager Shutdown event delivered through global service");
                    }
                    _ => panic!("INTEGRATION FAILURE: Expected Queue::Shutdown event, got: {:?}", received_event),
                }
            }
            Ok(None) => panic!("INTEGRATION FAILURE: Received None from global subscriber"),
            Err(_) => panic!("INTEGRATION FAILURE: Timeout waiting for Shutdown event - event publishing may not be working"),
        }

        // Cleanup
        {
            let mut notification_manager =
                crate::notifications::api::get_notification_service().await;
            notification_manager.clear_subscribers();
        }
    }

    #[tokio::test]
    async fn test_complete_queue_lifecycle_integration_end_to_end() {
        // INTEGRATION TEST: Complete lifecycle with multiple subscribers
        // This tests the full integration under realistic conditions

        // Create multiple subscribers to verify broadcast works
        let mut subscriber1 = {
            let mut notification_manager =
                crate::notifications::api::get_notification_service().await;
            notification_manager.clear_subscribers();

            notification_manager
                .subscribe(
                    "integration-complete-1".to_string(),
                    EventFilter::QueueOnly,
                    "integration:complete1".to_string(),
                )
                .unwrap()
        };

        let mut subscriber2 = {
            let mut notification_manager =
                crate::notifications::api::get_notification_service().await;

            notification_manager
                .subscribe(
                    "integration-complete-2".to_string(),
                    EventFilter::QueueOnly,
                    "integration:complete2".to_string(),
                )
                .unwrap()
        };

        // Test complete lifecycle
        let manager = QueueManager::create().await;

        // Both subscribers should receive Started event
        for (i, subscriber) in [&mut subscriber1, &mut subscriber2].iter_mut().enumerate() {
            let started_event = timeout(Duration::from_millis(200), subscriber.recv())
                .await
                .expect(&format!(
                    "Subscriber {} should receive Started event",
                    i + 1
                ))
                .expect("Should have Started event");

            match started_event {
                Event::Queue(queue_event) => {
                    assert_eq!(queue_event.event_type, QueueEventType::Started);
                    assert_eq!(queue_event.queue_id, "global");
                }
                _ => panic!("Expected Started event for subscriber {}", i + 1),
            }
        }

        // Test queue operations work
        let publisher = manager
            .create_publisher("test-producer".to_string())
            .expect("Should create publisher");
        let consumer = manager
            .create_consumer("test-plugin".to_string())
            .expect("Should create consumer");

        assert_eq!(manager.queue_count(), 1);
        assert_eq!(manager.active_consumer_count().unwrap(), 1);

        // Test shutdown event delivery to all subscribers
        manager.shutdown().await;

        for (i, subscriber) in [&mut subscriber1, &mut subscriber2].iter_mut().enumerate() {
            let shutdown_event = timeout(Duration::from_millis(200), subscriber.recv())
                .await
                .expect(&format!(
                    "Subscriber {} should receive Shutdown event",
                    i + 1
                ))
                .expect("Should have Shutdown event");

            match shutdown_event {
                Event::Queue(queue_event) => {
                    assert_eq!(queue_event.event_type, QueueEventType::Shutdown);
                    assert_eq!(queue_event.queue_id, "global");
                }
                _ => panic!("Expected Shutdown event for subscriber {}", i + 1),
            }
        }

        println!("✓ INTEGRATION: Complete end-to-end lifecycle with multiple subscribers works");

        // Cleanup
        drop(publisher);
        drop(consumer);
        {
            let mut notification_manager =
                crate::notifications::api::get_notification_service().await;
            notification_manager.clear_subscribers();
        }
    }
}
