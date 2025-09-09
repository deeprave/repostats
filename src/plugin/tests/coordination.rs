//! Integration tests for plugin coordination and lifecycle management
//!
//! These tests verify the RS-14 component coordination implementation works correctly,
//! including plugin completion detection, shutdown notifications, configurable timeouts,
//! and deadlock prevention.

#[cfg(test)]
mod integration_tests {
    use crate::core::cleanup::Cleanup;
    use crate::notifications::api::{
        Event, EventFilter, PluginEvent, PluginEventType, SystemEventType,
    };
    use crate::plugin::manager::{PluginManager, PluginManagerConfig};
    use crate::scanner::manager::ScannerManager;
    use std::time::Duration;
    use tokio::time::timeout;

    /// Test that plugin completion coordination works end-to-end
    #[tokio::test]
    async fn test_plugin_completion_coordination() {
        // Test scenario 1: No active plugins - should complete immediately
        let config = PluginManagerConfig::with_timeouts(
            Duration::from_millis(50), // completion_event_timeout
            Duration::from_secs(5),    // shutdown_timeout
            Duration::from_millis(25), // completion_check_interval
            Duration::from_secs(30),   // plugin_timeout
        )
        .expect("Should create valid config with valid timeouts");
        let mut plugin_manager = PluginManager::with_config(1, config);

        // Initialize plugin manager before calling await_all_plugins_completion
        plugin_manager
            .initialize()
            .await
            .expect("Should initialize plugin manager");

        // With no active plugins, this should return immediately
        let start_time = std::time::Instant::now();
        let result = plugin_manager.await_all_plugins_completion().await;
        let duration = start_time.elapsed();

        assert!(result.is_ok(), "Should succeed when no plugins are active");
        assert!(
            duration < Duration::from_millis(100),
            "Should complete quickly with no active plugins"
        );
    }

    /// Test plugin coordination with configurable timeouts
    #[tokio::test]
    async fn test_plugin_coordination_with_configurable_timeouts() {
        // This test verifies that the configurable timeouts are used correctly
        // when plugins are being monitored for completion

        let config = PluginManagerConfig::with_timeouts(
            Duration::from_millis(10), // Very short completion_event_timeout for testing
            Duration::from_millis(100), // Short shutdown_timeout
            Duration::from_millis(5),  // Short completion_check_interval
            Duration::from_secs(5),    // Minimum valid plugin_timeout for testing
        )
        .expect("Should create valid config with valid timeouts");
        let mut plugin_manager = PluginManager::with_config(1, config.clone());

        // Verify the configuration is applied correctly
        assert_eq!(
            plugin_manager.config.completion_event_timeout,
            Duration::from_millis(10)
        );
        assert_eq!(
            plugin_manager.config.shutdown_timeout,
            Duration::from_millis(100)
        );
        assert_eq!(
            plugin_manager.config.completion_check_interval,
            Duration::from_millis(5)
        );

        // Initialize plugin manager before calling await_all_plugins_completion
        plugin_manager
            .initialize()
            .await
            .expect("Should initialize plugin manager");

        // Test that the timeout configuration affects behavior
        // Since no plugins are registered, this will complete immediately
        let result = plugin_manager.await_all_plugins_completion().await;
        assert!(
            result.is_ok(),
            "Should succeed with custom timeout configuration"
        );
    }

    /// Test that shutdown notification mechanism works
    #[tokio::test]
    #[ignore = "Integration test that requires exclusive access to global notification service"]
    async fn test_shutdown_notification_mechanism() {
        let mut plugin_manager = PluginManager::new(1);

        // Set up plugin manager to listen for system events
        plugin_manager
            .setup_system_notification_subscriber()
            .await
            .expect("Should set up notification subscriber");

        // Get notification manager
        let mut notification_manager = crate::notifications::api::get_notification_service().await;

        // Create a subscriber to verify system shutdown events are published correctly
        let mut system_subscriber = notification_manager
            .subscribe(
                "test-system-listener".to_string(),
                EventFilter::SystemOnly,
                "TestSystemListener".to_string(),
            )
            .expect("Should create system event subscriber");

        // Simulate system shutdown notification
        let shutdown_event = crate::notifications::event::Event::System(
            crate::notifications::event::SystemEvent::new(SystemEventType::Shutdown),
        );

        notification_manager
            .publish(shutdown_event.clone())
            .await
            .expect("Should publish system shutdown event");

        // Verify that the system shutdown event is received
        let result = timeout(Duration::from_millis(500), system_subscriber.recv()).await;

        match result {
            Ok(Some(Event::System(system_event))) => {
                assert_eq!(system_event.event_type, SystemEventType::Shutdown);
                println!("✓ System shutdown event received and processed correctly");
            }
            Ok(Some(_)) => panic!("Received non-system event when expecting shutdown"),
            Ok(None) => panic!("Channel closed without receiving shutdown notification"),
            Err(_) => panic!("Timed out waiting for system shutdown event"),
        }

        // Verify plugin manager processes shutdown events without hanging
        // In a real scenario with registered plugins, they would receive shutdown notifications
        // For this test, we just verify the basic notification flow works
    }

    /// Test plugin completion tracking memory management
    #[tokio::test]
    async fn test_plugin_completion_memory_management() {
        let plugin_manager = PluginManager::new(1);

        // Initially, completion tracking should be empty
        {
            let completion = plugin_manager.plugin_completion.read().await;
            assert_eq!(completion.len(), 0);
        }

        // Add several plugins to completion tracking by marking them as completed
        let plugin_names = vec!["plugin-1", "plugin-2", "plugin-3"];

        for name in &plugin_names {
            plugin_manager.mark_plugin_completed(name).await;
        }

        // Check that all plugins are tracked and marked as completed
        {
            let completion = plugin_manager.plugin_completion.read().await;
            assert_eq!(completion.len(), plugin_names.len());

            for name in &plugin_names {
                assert!(completion.contains_key(*name));
                assert!(completion[*name]); // Should be true (completed)
            }
        }

        // Test cleanup functionality
        plugin_manager.cleanup_completed_plugins().await;

        // After cleanup, all completed plugins should be removed
        {
            let completion = plugin_manager.plugin_completion.read().await;
            assert_eq!(
                completion.len(),
                0,
                "Completed plugins should be cleaned up"
            );
        }

        // The memory management is verified by ensuring cleanup removes completed entries
    }

    /// Test that deadlock prevention works in scanner cleanup
    #[tokio::test]
    async fn test_scanner_cleanup_deadlock_prevention() {
        // Create a scanner manager
        let scanner_manager = ScannerManager::new();

        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("repostats_test_cleanup");
        std::fs::create_dir_all(&temp_dir).expect("Should create temp directory");

        // Simulate adding a checkout state
        // Note: This is testing the cleanup coordination, not the actual checkout functionality

        // Test that cleanup doesn't deadlock
        let start_time = std::time::Instant::now();

        // Call cleanup - this should complete without deadlocking
        scanner_manager.cleanup();

        let cleanup_duration = start_time.elapsed();

        // Cleanup should complete quickly (within reasonable time)
        assert!(cleanup_duration < Duration::from_secs(1));

        // Clean up test directory
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Test event-driven plugin completion flow
    #[tokio::test]
    #[ignore = "Integration test that requires exclusive access to global notification service"]
    async fn test_event_driven_plugin_completion() {
        let mut notification_manager = crate::notifications::api::get_notification_service().await;

        // Add setup delay to ensure notification system is fully ready
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Subscribe to plugin events to verify they can be sent and received
        let mut subscriber = notification_manager
            .subscribe(
                "test-plugin-events".to_string(),
                EventFilter::PluginOnly,
                "TestPluginEventSubscriber".to_string(),
            )
            .expect("Should create plugin event subscriber");

        // Add small delay after subscription to ensure it's active
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Publish a plugin completion event (simulating what dump plugin does)
        let completion_event = Event::Plugin(PluginEvent::with_message(
            PluginEventType::Completed,
            "test-plugin".to_string(),
            "test-scan-123".to_string(),
            "Test plugin completed successfully".to_string(),
        ));

        notification_manager
            .publish(completion_event.clone())
            .await
            .expect("Should publish plugin completion event");

        // Verify the event is received correctly with increased timeout for reliability
        let received_event = timeout(Duration::from_millis(500), subscriber.recv()).await;

        match received_event {
            Ok(Some(Event::Plugin(plugin_event))) => {
                assert_eq!(plugin_event.event_type, PluginEventType::Completed);
                assert_eq!(plugin_event.plugin_id, "test-plugin");
                assert!(plugin_event.message.is_some());
                assert!(plugin_event
                    .message
                    .unwrap()
                    .contains("completed successfully"));
            }
            Ok(Some(other_event)) => panic!("Received non-plugin event: {:?}", other_event),
            Ok(None) => panic!("Channel closed unexpectedly"),
            Err(timeout_err) => panic!("Event reception timed out after 500ms: {:?}", timeout_err),
        }

        // Cleanup to prevent state leakage between tests
        drop(subscriber);
        drop(notification_manager);
    }

    /// Test that plugin coordination handles concurrent operations
    #[tokio::test]
    async fn test_concurrent_plugin_coordination() {
        let plugin_manager = PluginManager::new(1);

        // Test concurrent marking of plugin completion
        let plugin_count = 5; // Reduced for reliability
        let plugin_names: Vec<String> = (0..plugin_count)
            .map(|i| format!("concurrent-plugin-{}", i))
            .collect();

        // Mark plugins as completed concurrently to test thread safety
        let plugin_manager_clone = std::sync::Arc::new(plugin_manager);
        let mut handles = Vec::new();

        for (i, plugin_name) in plugin_names.iter().enumerate() {
            let plugin_name = plugin_name.clone();
            let plugin_manager = plugin_manager_clone.clone();

            let handle = tokio::spawn(async move {
                // Small delay to simulate plugin work
                let delay_ms = (i as u64 * 13) % 50 + 10; // 10-60ms variation
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;

                plugin_manager.mark_plugin_completed(&plugin_name).await;
            });

            handles.push(handle);
        }

        // Wait for all concurrent operations to complete
        for handle in handles {
            handle
                .await
                .expect("Plugin marking task should complete successfully");
        }

        // Verify all plugins were marked as completed
        {
            let completion = plugin_manager_clone.plugin_completion.read().await;
            assert_eq!(completion.len(), plugin_count);

            for plugin_name in &plugin_names {
                assert!(completion.contains_key(plugin_name));
                assert!(completion[plugin_name]); // Should be completed
            }
        }

        // Get a mutable reference to the plugin manager for the final operations
        let mut plugin_manager = match std::sync::Arc::try_unwrap(plugin_manager_clone) {
            Ok(manager) => manager,
            Err(_) => panic!("Should be able to unwrap Arc when no other references exist"),
        };

        // Initialize plugin manager before calling await_all_plugins_completion
        plugin_manager
            .initialize()
            .await
            .expect("Should initialize plugin manager");

        // Test that await_all_plugins_completion works with the plugins
        let result = plugin_manager.await_all_plugins_completion().await;
        assert!(
            result.is_ok(),
            "Should succeed when all plugins are completed"
        );

        println!(
            "✓ Concurrent plugin coordination test passed with {} plugins",
            plugin_count
        );
    }
}
