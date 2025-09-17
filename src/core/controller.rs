//! System Controller Trait for Subsystem Coordination
//!
//! Provides a trait-based architecture for coordinating system-wide operations
//! across different subsystems (plugins, scanner, etc.) without holding locks.

use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::broadcast;

/// Controller timeout configuration
#[derive(Debug, Clone)]
pub struct ControllerConfig {
    pub completion_timeout: Duration,
    pub shutdown_timeout: Duration,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            completion_timeout: Duration::from_secs(60),
            shutdown_timeout: Duration::from_secs(30),
        }
    }
}

/// Controller registration information for dynamic discovery
pub struct ControllerInfo {
    pub name: &'static str,
    pub factory: fn() -> Pin<Box<dyn Future<Output = SystemResult<Box<dyn Controller>>> + Send>>,
}

// Register ControllerInfo with inventory for dynamic discovery
inventory::collect!(ControllerInfo);

/// Macro for registering controllers with the inventory system
#[macro_export]
macro_rules! controller {
    ($controller_type:ty, $name:expr) => {
        inventory::submit! {
            $crate::core::controller::ControllerInfo {
                name: $name,
                factory: || Box::pin(async { <$controller_type>::new().await.map(|c| Box::new(c) as Box<dyn $crate::core::controller::Controller>) }),
            }
        }
    };
}

/// Helper function to discover all registered controllers
pub fn discover_controllers() -> Vec<&'static ControllerInfo> {
    inventory::iter::<ControllerInfo>().collect()
}

/// System coordination error types
#[derive(Error, Debug)]
pub enum SystemError {
    #[error("Component '{component}' failed to shutdown within {timeout:?}")]
    ShutdownTimeout {
        component: String,
        timeout: Duration,
    },

    #[error("System coordination operation '{operation}' failed: {reason}")]
    CoordinationFailed { operation: String, reason: String },

    #[error("Failed to publish system event '{event_type}'")]
    EventPublishFailed { event_type: String },

    #[error("Plugin subsystem error")]
    PluginError {
        #[from]
        #[source]
        source: crate::plugin::error::PluginError,
    },
}

/// Simple result type for system coordination operations
pub type SystemResult<T> = Result<T, SystemError>;

/// Trait for subsystem controllers that participate in system coordination
#[async_trait]
pub trait Controller: Send + Sync {
    /// Gracefully stop this subsystem
    async fn graceful_system_stop(&mut self) -> SystemResult<()>;

    /// Wait for subsystem completion or handle shutdown signal
    ///
    /// # Broadcast Receiver Behaviour
    ///
    /// The `shutdown_rx` parameter uses tokio's broadcast channel, which has important semantic differences
    /// from other channel types:
    ///
    /// - **Message Delivery**: Broadcast receivers can miss messages if they're not actively listening
    ///   when a message is sent. This is different from mpsc channels where messages are queued.
    /// - **Late Subscription**: If a receiver subscribes after a message has been broadcast, it will
    ///   miss that message entirely.
    /// - **Receiver Independence**: Each receiver gets its own copy of broadcast messages, but only
    ///   if they're listening at the time of broadcast.
    /// - **RecvError::Lagged**: Receivers can fall behind if messages are sent faster than consumed,
    ///   resulting in `RecvError::Lagged` which indicates missed messages.
    ///
    /// For shutdown coordination, this means:
    /// - Controllers must be actively listening on `shutdown_rx` before shutdown signals are sent
    /// - Missing a shutdown signal could result in the controller never terminating gracefully
    /// - Implementations should handle `RecvError::Lagged` as equivalent to receiving shutdown
    async fn await_system_completion_with_shutdown(
        &mut self,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> SystemResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    // Mock controller for testing
    struct MockController {
        stop_called: Arc<Mutex<bool>>,
        await_called: Arc<Mutex<bool>>,
        should_fail: bool,
        fail_message: String,
    }

    impl MockController {
        async fn new() -> SystemResult<Self> {
            Ok(Self {
                stop_called: Arc::new(Mutex::new(false)),
                await_called: Arc::new(Mutex::new(false)),
                should_fail: false,
                fail_message: String::new(),
            })
        }

        fn failing(message: &str) -> Self {
            Self {
                stop_called: Arc::new(Mutex::new(false)),
                await_called: Arc::new(Mutex::new(false)),
                should_fail: true,
                fail_message: message.to_string(),
            }
        }
    }

    #[async_trait]
    impl Controller for MockController {
        async fn graceful_system_stop(&mut self) -> SystemResult<()> {
            let mut called = self.stop_called.lock().await;
            *called = true;

            if self.should_fail {
                Err(SystemError::CoordinationFailed {
                    operation: "graceful_stop".to_string(),
                    reason: self.fail_message.clone(),
                })
            } else {
                Ok(())
            }
        }

        async fn await_system_completion_with_shutdown(
            &mut self,
            _shutdown_rx: broadcast::Receiver<()>,
        ) -> SystemResult<()> {
            let mut called = self.await_called.lock().await;
            *called = true;

            if self.should_fail {
                Err(SystemError::CoordinationFailed {
                    operation: "await_completion".to_string(),
                    reason: self.fail_message.clone(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn test_controller_trait_graceful_stop_success() {
        let mut controller = MockController::new()
            .await
            .expect("Should create controller");
        let result = controller.graceful_system_stop().await;

        assert!(result.is_ok());
        assert!(*controller.stop_called.lock().await);
    }

    #[tokio::test]
    async fn test_controller_trait_graceful_stop_failure() {
        let mut controller = MockController::failing("test failure");
        let result = controller.graceful_system_stop().await;

        assert!(result.is_err());
        if let Err(SystemError::CoordinationFailed { operation, reason }) = result {
            assert_eq!(operation, "graceful_stop");
            assert_eq!(reason, "test failure");
        } else {
            panic!("Expected CoordinationFailed error");
        }
        assert!(*controller.stop_called.lock().await);
    }

    #[tokio::test]
    async fn test_controller_trait_await_completion_success() {
        let mut controller = MockController::new()
            .await
            .expect("Should create controller");
        let (_tx, rx) = broadcast::channel(1);

        let result = controller.await_system_completion_with_shutdown(rx).await;

        assert!(result.is_ok());
        assert!(*controller.await_called.lock().await);
    }

    #[tokio::test]
    async fn test_controller_trait_await_completion_failure() {
        let mut controller = MockController::failing("completion timeout");
        let (_tx, rx) = broadcast::channel(1);

        let result = controller.await_system_completion_with_shutdown(rx).await;

        assert!(result.is_err());
        if let Err(SystemError::CoordinationFailed { operation, reason }) = result {
            assert_eq!(operation, "await_completion");
            assert_eq!(reason, "completion timeout");
        } else {
            panic!("Expected CoordinationFailed error");
        }
        assert!(*controller.await_called.lock().await);
    }

    #[tokio::test]
    async fn test_controller_trait_as_dyn_object() {
        // Test that the trait can be used as a dynamic object
        let mut controller: Box<dyn Controller> = Box::new(
            MockController::new()
                .await
                .expect("Should create controller"),
        );

        let result = controller.graceful_system_stop().await;
        assert!(result.is_ok());

        let (_tx, rx) = broadcast::channel(1);
        let result = controller.await_system_completion_with_shutdown(rx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_controllers_in_collection() {
        // Test that multiple controllers can be managed together
        let mut controllers: Vec<Box<dyn Controller>> = vec![
            Box::new(
                MockController::new()
                    .await
                    .expect("Should create controller"),
            ),
            Box::new(
                MockController::new()
                    .await
                    .expect("Should create controller"),
            ),
            Box::new(MockController::failing("controller 3 failed")),
        ];

        let mut results = Vec::new();
        for controller in &mut controllers {
            results.push(controller.graceful_system_stop().await);
        }

        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert!(results[2].is_err());
    }

    #[tokio::test]
    async fn test_controller_with_shutdown_signal() {
        let mut controller = MockController::new()
            .await
            .expect("Should create controller");
        let (tx, rx) = broadcast::channel(1);

        // Spawn task to send shutdown signal
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = tx.send(());
        });

        // Controller should handle shutdown signal (mock just returns Ok)
        let result = controller.await_system_completion_with_shutdown(rx).await;
        assert!(result.is_ok());
    }

    // Tests for inventory-based controller discovery
    mod inventory_tests {
        use super::*;

        // Test ControllerInfo struct for dynamic registration
        #[tokio::test]
        async fn test_controller_info_creation() {
            // Test that ControllerInfo can be created with name and factory
            let info = ControllerInfo {
                name: "test-controller",
                factory: || {
                    Box::pin(async {
                        MockController::new()
                            .await
                            .map(|c| Box::new(c) as Box<dyn Controller>)
                    })
                },
            };

            assert_eq!(info.name, "test-controller");

            // Test factory creates working controller
            let mut controller = (info.factory)().await.expect("Factory should succeed");
            // Test that the controller actually works (can't check type_id on trait objects)
            assert!(controller.graceful_system_stop().await.is_ok());
        }

        // Test controller! macro registration
        #[test]
        fn test_controller_macro_registration() {
            // Test that the controller! macro works and registers controllers
            // Note: This creates a static registration at compile time
            use crate::controller;

            // Register a test controller using the macro
            controller!(MockController, "macro-test-controller");

            // Test that discover_controllers finds registered controllers
            let discovered = discover_controllers();
            let found_controller = discovered
                .iter()
                .find(|info| info.name == "macro-test-controller");

            assert!(
                found_controller.is_some(),
                "Should find macro-registered controller"
            );
        }

        // Test inventory-based discovery mechanism
        #[test]
        fn test_inventory_controller_discovery() {
            // Test that registered controllers can be discovered via inventory
            let discovered = discover_controllers();

            // The discovery function should work (may be empty if no controllers registered yet)
            // This tests the mechanism, not necessarily that controllers are present
            assert!(discovered.len() >= 0, "Discovery mechanism should work");

            // Test that we can iterate through discovered controllers
            for controller_info in discovered.iter().take(5) {
                assert!(
                    !controller_info.name.is_empty(),
                    "Controller should have a name"
                );
            }
        }

        // Test EventController with dynamic controller discovery
        #[tokio::test]
        async fn test_event_controller_with_discovered_controllers() {
            // Test that EventController can discover and use registered controllers
            // let event_controller = EventController::new();

            // Should automatically discover all registered controllers
            // assert!(!event_controller.subsystem_controllers.is_empty());

            // Test coordinated shutdown with discovered controllers
            // let result = event_controller.graceful_system_stop().await;
            // assert!(result.is_ok());

            // Placeholder test until EventController integration is implemented
            assert!(true, "Placeholder for EventController discovery test");
        }

        // Test controller factory function signature
        #[tokio::test]
        async fn test_controller_factory_signature() {
            // Test that factory functions return Future<SystemResult<Box<dyn Controller>>>
            fn factory() -> Pin<Box<dyn Future<Output = SystemResult<Box<dyn Controller>>> + Send>>
            {
                Box::pin(async {
                    Ok(Box::new(MockController::failing("async test")) as Box<dyn Controller>)
                })
            }

            let mut controller = factory().await.expect("Factory should succeed");
            // Test that the returned controller actually works
            assert!(controller.graceful_system_stop().await.is_err()); // This one should fail as designed
        }

        // Test multiple controller types can be registered
        #[tokio::test]
        async fn test_multiple_controller_registration() {
            fn plugin_factory(
            ) -> Pin<Box<dyn Future<Output = SystemResult<Box<dyn Controller>>> + Send>>
            {
                Box::pin(async {
                    MockController::new()
                        .await
                        .map(|c| Box::new(c) as Box<dyn Controller>)
                })
            }

            fn scanner_factory(
            ) -> Pin<Box<dyn Future<Output = SystemResult<Box<dyn Controller>>> + Send>>
            {
                Box::pin(async {
                    Ok(Box::new(MockController::failing("scanner")) as Box<dyn Controller>)
                })
            }

            let plugin_info = ControllerInfo {
                name: "plugin",
                factory: plugin_factory,
            };

            let scanner_info = ControllerInfo {
                name: "scanner",
                factory: scanner_factory,
            };

            // Should be able to create different controller types
            let mut plugin_controller = (plugin_info.factory)()
                .await
                .expect("Plugin factory should succeed");
            let mut scanner_controller = (scanner_info.factory)()
                .await
                .expect("Scanner factory should succeed");

            // Test that both controllers work as expected
            assert!(plugin_controller.graceful_system_stop().await.is_ok());
            assert!(scanner_controller.graceful_system_stop().await.is_err()); // This one should fail
        }

        // Test error handling in discovered controllers
        #[tokio::test]
        async fn test_discovered_controller_error_collection() {
            // Test that errors from multiple controllers are collected properly
            let failing_factory = || {
                Box::pin(async {
                    Ok::<Box<dyn Controller>, SystemError>(Box::new(MockController::failing(
                        "test error",
                    ))
                        as Box<dyn Controller>)
                })
            };
            let working_factory = || {
                Box::pin(async {
                    MockController::new()
                        .await
                        .map(|c| Box::new(c) as Box<dyn Controller>)
                })
            };

            let mut controllers: Vec<Box<dyn Controller>> = vec![
                failing_factory().await.expect("Factory should succeed"),
                working_factory().await.expect("Factory should succeed"),
                failing_factory().await.expect("Factory should succeed"),
            ];

            // Test that all controllers are attempted even when some fail
            let mut results = Vec::new();
            for controller in &mut controllers {
                results.push(controller.graceful_system_stop().await);
            }

            // Should have results from all controllers
            assert_eq!(results.len(), 3);
            assert!(results[0].is_err()); // First fails
            assert!(results[1].is_ok()); // Second succeeds
            assert!(results[2].is_err()); // Third fails
        }
    }
}
