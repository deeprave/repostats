//! Integration tests for application services

use crate::notifications::api::{get_notification_service, notification_service};
use crate::plugin::api::{get_plugin_service, plugin_service};
use crate::queue::api::{get_queue_service, queue_service};

#[tokio::test]
async fn test_service_initialization() {
    // Test that notification service is accessible
    let count = notification_service().subscriber_count().await;
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
                let _ = notification_service().subscriber_count().await;

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
    // Test that we can create publishers and consumers through the stable facade
    let publisher = queue_service()
        .create_publisher("test-producer".to_string())
        .unwrap();
    let consumer = queue_service()
        .create_consumer("test-plugin".to_string())
        .unwrap();

    assert_eq!(publisher.producer_id(), "test-producer");
    assert_eq!(consumer.plugin_name(), "test-plugin");

    // Test multiple producer support
    let publisher2 = queue_service()
        .create_publisher("another-producer".to_string())
        .unwrap();
    assert_eq!(publisher2.producer_id(), "another-producer");
}

#[tokio::test]
async fn test_plugin_manager_access() {
    assert_eq!(
        plugin_service().api_version().await,
        crate::core::version::get_api_version()
    );

    assert!(plugin_service().get_active_plugins().await.is_empty());
}

#[tokio::test]
async fn test_facades_preserve_existing_global_services() {
    let facade_subscriber_count = notification_service().subscriber_count().await;
    let direct_subscriber_count = get_notification_service().await.subscriber_count();
    assert_eq!(facade_subscriber_count, direct_subscriber_count);

    let direct_plugin_api_version = get_plugin_service().await.api_version();
    let facade_plugin_api_version = plugin_service().api_version().await;
    assert_eq!(facade_plugin_api_version, direct_plugin_api_version);

    let direct_queue = get_queue_service();
    let facade_queue = queue_service().manager();
    assert!(std::sync::Arc::ptr_eq(&direct_queue, &facade_queue));
}
