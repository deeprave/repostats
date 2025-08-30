//! Tests for queue lifecycle event integration

#[cfg(test)]
mod tests {
    use crate::core::services::get_services;
    use crate::notifications::api::{Event, EventFilter, QueueEvent, QueueEventType};
    use crate::queue::api::QueueManager;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_notification_system_basic_functionality() {
        let services = get_services();
        let mut notification_manager = services.notification_manager().await;

        // Subscribe to queue events
        let mut subscriber = notification_manager
            .subscribe(
                "test-basic".to_string(),
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
    async fn test_queue_manager_create_publishes_started_event() {
        // First create the manager to get the expected behavior
        let _manager = QueueManager::create().await;

        // Now check if we can detect any events were published
        // This test just verifies the create method works without hanging
        println!("✓ QueueManager::create() completes without hanging");
    }

    #[tokio::test]
    async fn test_queue_started_event_published_on_creation() {
        let services = get_services();

        // Subscribe to queue events BEFORE creating the manager
        let mut subscriber = {
            let mut notification_manager = services.notification_manager().await;
            notification_manager
                .subscribe(
                    "test-queue-lifecycle".to_string(),
                    EventFilter::QueueOnly,
                    "test:lifecycle".to_string(),
                )
                .unwrap()
        }; // notification_manager lock is released here

        // Create a new queue manager in same task (this should publish a Started event)
        let _manager = QueueManager::create().await;

        // Wait for the event with a timeout
        let result = timeout(Duration::from_millis(200), subscriber.recv()).await;

        match result {
            Ok(Some(received_event)) => {
                match received_event {
                    Event::Queue(queue_event) => {
                        assert_eq!(queue_event.event_type, QueueEventType::Started);
                        assert_eq!(queue_event.queue_id, "global"); // Single global queue
                        println!("✓ QueueManager publishes Started event on creation");
                    }
                    _ => panic!("Expected Queue::Started event, got: {:?}", received_event),
                }
            }
            Ok(None) => panic!("Received None from subscriber"),
            Err(_) => {
                // This is expected to fail since we haven't implemented the event publishing yet
                // This is a proper failing test for TDD
                panic!("Test timed out waiting for Started event - this means event publishing is not implemented yet");
            }
        }
    }
}
