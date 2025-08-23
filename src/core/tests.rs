//! Tests for the core services module

use super::services::{get_services, SERVICES};

#[tokio::test]
async fn test_service_registry_initialization() {
    let services = get_services();

    // Test that notification manager is accessible
    let notification_manager = services.notification_manager();

    // The notification manager is wrapped in the ServiceRegistry
    // We need to test that we can access its functionality
    assert_eq!(notification_manager.subscriber_count(), 0);
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

#[test]
fn test_concurrent_service_access() {
    // Test that multiple threads can access services concurrently
    let handles: Vec<_> = (0..10).map(|i| {
        std::thread::spawn(move || {
            let services = get_services();
            let _notification_manager = services.notification_manager();

            // Each thread gets the notification manager
            // Return the thread ID to verify all completed
            i
        })
    }).collect();

    // Wait for all threads to complete
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.join().unwrap());
    }

    // All 10 threads should complete
    results.sort();
    assert_eq!(results, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
}