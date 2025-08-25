//! Tests for the core services module

use super::services::{get_services, SERVICES};

#[tokio::test]
async fn test_service_registry_initialization() {
    let services = get_services();

    // Test that notification manager is accessible
    let notification_manager = services.notification_manager().await;

    // The notification manager is wrapped in the ServiceRegistry
    // We need to test that we can access its functionality
    let count = notification_manager.subscriber_count();
    // Don't assert specific count since tests may run in parallel and share state
    println!(
        "Service registry initialization test: current subscriber count: {}",
        count
    );
}

#[test]
fn test_lazy_lock_singleton_behavior() {
    // Test that SERVICES is a singleton
    let services1 = get_services();
    let services2 = get_services();

    // They should be the same instance (same address)
    assert!(std::ptr::eq(services1, services2));

    // Direct access should also be the same
    let services3 = &*SERVICES;
    assert!(std::ptr::eq(services1, services3));
}

#[tokio::test]
async fn test_async_notification_manager_access() {
    let services = get_services();

    // Test async access works properly
    {
        let manager = services.notification_manager().await;
        let count = manager.subscriber_count();
        println!("Async access works, current subscriber count: {}", count);
    }

    // Test multiple async access calls work consistently
    let async_count1 = services.notification_manager().await.subscriber_count();
    let async_count2 = services.notification_manager().await.subscriber_count();
    assert_eq!(async_count1, async_count2);
}

#[tokio::test]
async fn test_concurrent_service_access() {
    use tokio::task;

    // Test that multiple async tasks can access services concurrently
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            task::spawn(async move {
                let services = get_services();
                let _notification_manager = services.notification_manager().await;

                // Each task gets the notification manager
                // Return the task ID to verify all completed
                i
            })
        })
        .collect();

    // Wait for all tasks to complete
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }

    // All 10 tasks should complete
    results.sort();
    assert_eq!(results, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
}

#[tokio::test]
async fn test_queue_manager_access_via_service_registry() {
    let services = get_services();

    // Test that queue manager is accessible via ServiceRegistry
    let queue_manager = services.queue_manager();

    // Test that we can create publishers and consumers through the global service
    let publisher = queue_manager
        .create_publisher("test-producer".to_string())
        .unwrap();
    let consumer = queue_manager
        .create_consumer("test-plugin".to_string())
        .unwrap();

    assert_eq!(publisher.producer_id(), "test-producer");
    assert_eq!(consumer.plugin_name(), "test-plugin");

    // Test multiple producer support
    let publisher2 = queue_manager
        .create_publisher("another-producer".to_string())
        .unwrap();
    assert_eq!(publisher2.producer_id(), "another-producer");
}
