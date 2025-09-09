//! Integration tests for application services

use crate::notifications::api::get_notification_service;
use crate::plugin::api::get_plugin_service;
use crate::queue::api::get_queue_service;

#[tokio::test]
async fn test_service_initialization() {
    // Test that notification service is accessible
    let notification_manager = get_notification_service().await;
    let count = notification_manager.subscriber_count();
    // Don't assert specific count since tests may run in parallel and share state
    println!(
        "Service initialization test: current subscriber count: {}",
        count
    );
}

#[test]
fn test_queue_service_singleton_behavior() {
    // Test that queue service returns same instance
    let queue_service1 = get_queue_service();
    let queue_service2 = get_queue_service();

    // Should be same Arc instance
    assert!(std::sync::Arc::ptr_eq(&queue_service1, &queue_service2));
}

#[tokio::test]
async fn test_concurrent_service_access() {
    use tokio::task;

    // Test that multiple async tasks can access services concurrently
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            task::spawn(async move {
                let _notification_manager = get_notification_service().await;

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
async fn test_queue_manager_access() {
    // Test that queue manager is accessible
    let queue_manager = get_queue_service();

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

#[tokio::test]
async fn test_plugin_manager_access() {
    // Test that plugin manager is accessible
    let plugin_manager = get_plugin_service().await;

    // Test that plugin manager has the correct API version
    assert_eq!(
        plugin_manager.api_version(),
        crate::core::version::get_api_version()
    );

    // Test that we can access the plugin registry through the manager
    let registry = plugin_manager.registry();
    let plugin_count = registry.plugin_count().await;

    // Initially should be empty
    assert_eq!(plugin_count, 0);
}
