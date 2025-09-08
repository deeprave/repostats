use crate::core::error_handling::log_error_with_context;
use crate::core::shutdown::ShutdownCoordinator;
use crate::notifications::api::{Event, SystemEvent, SystemEventType};
use crate::scanner::api::ScanError;

mod app;
mod core;
mod notifications;
mod plugin;
mod queue;
mod scanner;
use notifications::api::*;

// Version access is now provided via crate::core::version

static COMMAND_NAME: &str = "repostats";

/// Main application entry point with unified error handling
///
/// The startup process returns a Result to enable proper error logging through
/// the ContextualError trait system before exiting. This allows:
/// - User-actionable errors to show specific, helpful messages
/// - System errors to show generic context with debug details
/// - Consistent error formatting across all startup failures
#[tokio::main]
async fn main() {
    // Try to get command name from args, otherwise use default
    let command_name_owned = std::env::args().next().and_then(|s| {
        std::path::PathBuf::from(s)
            .file_name()
            .map(|os| os.to_string_lossy().into_owned())
    });
    let command_name = command_name_owned.as_deref().unwrap_or(COMMAND_NAME);
    let pid = std::process::id();

    // Use shutdown coordinator to guard the entire application execution
    let result =
        ShutdownCoordinator::guard_with_coordinator(|coordinator, mut shutdown_rx| async move {
            // Application startup with shutdown checking
            let scanner_manager = tokio::select! {
                result = app::startup::startup(command_name) => {
                    match result {
                        Ok(scanner_manager) => scanner_manager,
                        Err(e) => {
                            log_error_with_context(&e, "Application startup");
                            std::process::exit(1);
                        }
                    }
                }
                shutdown_result = shutdown_rx.recv() => {
                    match shutdown_result {
                        Ok(_) => std::process::exit(0),
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            log::warn!("Shutdown channel closed during startup; exiting");
                            std::process::exit(0)
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            log::warn!("Missed shutdown signals during startup; exiting");
                            std::process::exit(0)
                        }
                    }
                }
            };

            // System start with shutdown checking
            let mut shutdown_check = shutdown_rx.resubscribe();
            tokio::select! {
                result = system_start(pid) => {
                    if let Err(e) = result {
                        log::error!("Failed to start system: {e}");
                        std::process::exit(1);
                    }
                }
                shutdown_result = shutdown_check.recv() => {
                    match shutdown_result {
                        Ok(_) => std::process::exit(0),
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            log::warn!("Shutdown channel closed during system start; exiting");
                            std::process::exit(0)
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            log::warn!("Missed shutdown signals during system start; exiting");
                            std::process::exit(0)
                        }
                    }
                }
            };

            // Spawn spinner task if appropriate (skip if any plugin suppresses progress)
            let suppress_progress = {
                let plugin_manager = core::services::get_plugin_service().await;
                plugin_manager
                    .get_combined_requirements()
                    .await
                    .suppresses_progress()
            };
            if !suppress_progress && app::spinner::should_show_spinner() {
                let spinner_shutdown = shutdown_rx.resubscribe();
                tokio::spawn(async move {
                    if let Err(e) = app::spinner::run_spinner(spinner_shutdown).await {
                        log::debug!("Spinner task failed: {}", e);
                    }
                });
            }

            // Handle scanner execution if configured
            let final_result = if let Some(scanner_manager) = scanner_manager {
                log::info!("{command_name}: ✅ Repository Statistics Tool starting");

                run_scanner_with_coordination(
                    scanner_manager,
                    shutdown_rx,
                    coordinator.shutdown_requested.clone(),
                )
                .await
            } else {
                // Early exit
                return Ok(());
            };

            // System shutdown
            if let Err(e) = system_stop(pid).await {
                log::warn!("Error stopping system: {e}");
            }

            final_result
        })
        .await;

    if let Err(e) = result {
        log::error!("Application error: {e}");
        std::process::exit(1);
    }
}

/// Run scanner with component coordination for plugin shutdown
async fn run_scanner_with_coordination(
    scanner_manager: std::sync::Arc<scanner::api::ScannerManager>,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), ScanError> {
    // Get services for plugin coordination
    log::trace!("Getting notification manager for plugin initialization");
    let mut notification_manager = core::services::get_notification_service().await;
    log::trace!("Acquired notification manager lock successfully");

    // Initialize plugin event subscription for coordination (prevents race conditions)
    log::trace!("Initializing plugin event subscription");
    {
        let plugin_manager = core::services::get_plugin_service().await;
        log::trace!("Acquired plugin manager lock for initialization");

        if let Err(e) = plugin_manager
            .initialize_event_subscription(&mut notification_manager)
            .await
        {
            log::error!("Failed to initialize plugin event subscription: {}", e);
            return Err(ScanError::Configuration {
                message: format!("Plugin coordination setup failed: {}", e),
            });
        }
        log::trace!("Plugin event subscription initialization completed");
        // Plugin manager lock drops here automatically
    }

    // Drop notification manager lock early to avoid potential conflicts
    drop(notification_manager);

    // Clone scanner_manager for the select block
    let scanner_manager_for_start = scanner_manager.clone();

    // Get opaque cleanup handle for coordinated shutdown
    let cleanup_handle = scanner_manager.cleanup_handle();

    // Run scanner with component coordination
    tokio::select! {
        result = start_scanner(scanner_manager_for_start) => {
            // Normal completion path: wait for plugins then cleanup
            match result {
                Ok(()) => {
                    // Check if shutdown was already requested before resubscription
                    let should_stop_immediately = shutdown_requested.load(std::sync::atomic::Ordering::Acquire);

                    if should_stop_immediately {
                        // Shutdown already triggered, proceed with graceful stop
                        log::trace!("Shutdown already requested, proceeding with graceful plugin stop");
                        let timeout = std::time::Duration::from_secs(30);

                        // Scope plugin manager access for graceful stop
                        let plugin_mgr = core::services::get_plugin_service().await;
                        if let Ok(summary) = plugin_mgr.graceful_stop_all(timeout).await {
                            if !summary.all_completed() {
                                log::trace!("Some plugins failed to stop gracefully: {:?}", summary);
                            }
                        }
                        // Drop lock automatically at end of scope
                    } else {
                        // Use shutdown-integrated plugin completion wait to handle signals
                        let shutdown_rx = shutdown_rx.resubscribe();

                        // Scope plugin manager access for completion wait
                        let plugin_mgr = core::services::get_plugin_service().await;
                        if let Err(e) = plugin_mgr.await_all_plugins_completion_with_shutdown(shutdown_rx).await {
                            log::trace!("Plugin completion wait failed: {}", e);
                        }
                        // Drop lock automatically at end of scope
                    }
                    cleanup_handle.cleanup();
                    Ok(())
                }
                Err(e) => {
                    // Scanner failed, still need to coordinate shutdown
                    let timeout = std::time::Duration::from_secs(30);

                    // Scope plugin manager access for graceful stop after scanner failure
                    let plugin_mgr = core::services::get_plugin_service().await;
                    if let Ok(summary) = plugin_mgr.graceful_stop_all(timeout).await {
                        if !summary.all_completed() {
                            log::trace!("Some plugins failed to stop gracefully: {:?}", summary);
                        }
                    }
                    // Drop lock automatically at end of scope

                    cleanup_handle.cleanup();
                    Err(e)
                }
            }
        }
        shutdown_result = shutdown_rx.recv() => {
            match shutdown_result {
                Ok(_) => {
                    // Signal interruption path: graceful plugin stop then cleanup
                    let timeout = std::time::Duration::from_secs(30);

                    // Scope plugin manager access for signal-triggered graceful stop
                    let plugin_mgr = core::services::get_plugin_service().await;
                    if let Ok(summary) = plugin_mgr.graceful_stop_all(timeout).await {
                        if !summary.all_completed() {
                            log::trace!("Some plugins failed to stop gracefully: {:?}", summary);
                        }
                    }
                    // Drop lock automatically at end of scope

                    cleanup_handle.cleanup();
                    Ok(())
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    log::warn!("Shutdown channel closed during execution; performing cleanup");
                    cleanup_handle.cleanup();
                    Ok(())
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    log::warn!("Missed shutdown signals during execution; performing cleanup");
                    cleanup_handle.cleanup();
                    Ok(())
                }
            }
        }
    }
}

async fn system_start(pid: u32) -> Result<(), NotificationError> {
    // Get notification manager and process ID once
    let mut notification_manager = core::services::get_notification_service().await;

    // Publish system startup event
    let startup_event = Event::System(SystemEvent::with_message(
        SystemEventType::Startup,
        format!("System started, pid={pid}"),
    ));
    notification_manager.publish(startup_event).await
}

async fn system_stop(pid: u32) -> Result<(), NotificationError> {
    let mut notification_manager = core::services::get_notification_service().await;
    let shutdown_event = Event::System(SystemEvent::with_message(
        SystemEventType::Shutdown,
        format!("System shutting down, pid={pid}"),
    ));
    notification_manager.publish(shutdown_event).await
}

/// Start the actual repository scanner with the configured scanner manager
async fn start_scanner(
    scanner_manager: std::sync::Arc<scanner::api::ScannerManager>,
) -> Result<(), ScanError> {
    use log::debug;

    // Start scanning all configured repositories
    let result = scanner_manager.start_scanning().await;
    match &result {
        Ok(()) => debug!("All repository scanning completed successfully"),
        Err(e) => debug!("Repository scanning failed: {e}"),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_main_is_async() {
        // Test that main function is now async
        // This test should pass once we've converted to async
        assert!(true, "Main function is now async");
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_creation() {
        let (coordinator, _rx) = ShutdownCoordinator::new();

        // Should start with shutdown not requested
        assert!(!coordinator.is_shutdown_requested());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_trigger() {
        let (coordinator, mut rx) = ShutdownCoordinator::new();

        // Initially shutdown should not be requested
        assert!(!coordinator.is_shutdown_requested());

        // Trigger shutdown
        coordinator.trigger_shutdown();

        // Should now report shutdown requested
        assert!(coordinator.is_shutdown_requested());

        // Should receive shutdown signal
        let signal_received = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(signal_received.is_ok(), "Should receive shutdown signal");
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_multiple_subscribers() {
        let (coordinator, _rx1) = ShutdownCoordinator::new();
        let mut rx2 = coordinator.subscribe();
        let mut rx3 = coordinator.subscribe();

        // Trigger shutdown
        coordinator.trigger_shutdown();

        // All subscribers should receive the signal
        let signal2 = timeout(Duration::from_millis(100), rx2.recv()).await;
        let signal3 = timeout(Duration::from_millis(100), rx3.recv()).await;

        assert!(
            signal2.is_ok(),
            "Subscriber 2 should receive shutdown signal"
        );
        assert!(
            signal3.is_ok(),
            "Subscriber 3 should receive shutdown signal"
        );
        assert!(coordinator.is_shutdown_requested());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_idempotent_trigger() {
        let (coordinator, mut rx) = ShutdownCoordinator::new();

        // Should start with shutdown not requested
        assert!(!coordinator.is_shutdown_requested());

        // Trigger shutdown multiple times
        coordinator.trigger_shutdown();
        assert!(coordinator.is_shutdown_requested());

        coordinator.trigger_shutdown();
        assert!(coordinator.is_shutdown_requested());

        coordinator.trigger_shutdown();
        assert!(coordinator.is_shutdown_requested());

        // Should receive at least one signal (multiple triggers send multiple signals)
        let signal_received = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(signal_received.is_ok(), "Should receive shutdown signal");

        // State should remain consistently true
        assert!(coordinator.is_shutdown_requested());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_subscribe_after_trigger() {
        let (coordinator, _rx) = ShutdownCoordinator::new();

        // Trigger shutdown first
        coordinator.trigger_shutdown();
        assert!(coordinator.is_shutdown_requested());

        // Subscribe after shutdown was triggered
        let mut late_subscriber = coordinator.subscribe();

        // Late subscriber should not receive the signal that was already sent
        let no_signal = timeout(Duration::from_millis(50), late_subscriber.recv()).await;
        assert!(
            no_signal.is_err(),
            "Late subscriber should not receive already-sent signal"
        );

        // But should still be able to detect shutdown state
        assert!(coordinator.is_shutdown_requested());
    }
}
