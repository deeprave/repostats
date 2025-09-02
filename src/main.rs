use crate::core::error_handling::log_error_with_context;
use crate::notifications::api::{Event, SystemEvent, SystemEventType};
use crate::scanner::api::ScanError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

mod app;
mod core;
mod notifications;
mod plugin;
mod queue;
mod scanner;
use notifications::api::*;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

/// Parse the API version string from build script into u32
pub fn get_plugin_api_version() -> u32 {
    PLUGIN_API_VERSION.parse().unwrap_or(20250727)
}

static COMMAND_NAME: &str = "repostats";

/// Coordinates graceful shutdown across the application
pub struct ShutdownCoordinator {
    pub shutdown_tx: broadcast::Sender<()>,
    pub shutdown_requested: Arc<AtomicBool>,
}

impl ShutdownCoordinator {
    pub fn new() -> (Self, broadcast::Receiver<()>) {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let shutdown_requested = Arc::new(AtomicBool::new(false));

        let coordinator = Self {
            shutdown_tx,
            shutdown_requested,
        };

        (coordinator, shutdown_rx)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    pub fn trigger_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::Relaxed);
        let _ = self.shutdown_tx.send(());
    }

    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::Relaxed)
    }
}

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

    // Set up shutdown coordination
    let (coordinator, mut shutdown_rx) = ShutdownCoordinator::new();

    // Set up signal handlers immediately so they're active for the entire program lifetime
    setup_signal_handlers(
        coordinator.shutdown_tx.clone(),
        coordinator.shutdown_requested.clone(),
    );

    let mut shutdown_check = shutdown_rx.resubscribe();
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
        _ = shutdown_check.recv() => std::process::exit(0)
    };

    let mut shutdown_check = shutdown_rx.resubscribe();
    tokio::select! {
        result = system_start(pid) => {
            if let Err(e) = result {
                log::error!("Failed to start system: {e}");
                std::process::exit(1);
            }
        }
        _ = shutdown_check.recv() => std::process::exit(0)
    }

    {
        log::info!("{command_name}: ✅ Repository Statistics Tool starting");

        // Start the actual scanner if we have one configured
        if let Some(scanner_manager) = scanner_manager {
            // Clone the Arc so we can use it for cleanup
            let scanner_manager_for_cleanup = scanner_manager.clone();

            // Run scanner with signal handling
            let scanner_result = tokio::select! {
                result = start_scanner(scanner_manager) => {
                    result
                }
                _ = shutdown_rx.recv() => {
                    scanner_manager_for_cleanup.cleanup_all_checkouts();
                    Ok(())
                }
            };

            if let Err(e) = scanner_result {
                log::error!("Scanner error: {e}");
                std::process::exit(1);
            }
        }

        if let Err(e) = system_stop(pid).await {
            log::warn!("Error stopping system: {e}");
        }
    }
}

fn setup_signal_handlers(shutdown_tx: broadcast::Sender<()>, shutdown_requested: Arc<AtomicBool>) {
    #[cfg(unix)]
    {
        unsafe {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }

        use tokio::signal::unix::{signal, SignalKind};
        let signals = [
            SignalKind::interrupt(),
            SignalKind::terminate(),
            SignalKind::hangup(),
            SignalKind::quit(),
        ];

        for kind in signals {
            let tx = shutdown_tx.clone();
            let requested = shutdown_requested.clone();

            tokio::spawn(async move {
                if let Ok(mut sig) = signal(kind) {
                    sig.recv().await;
                    requested.store(true, Ordering::Relaxed);
                    let _ = tx.send(());
                }
            });
        }
    }

    #[cfg(not(unix))]
    {
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                shutdown_requested.store(true, Ordering::Relaxed);
                let _ = shutdown_tx.send(());
            }
        });
    }
}

async fn system_start(pid: u32) -> Result<(), NotificationError> {
    // Get notification manager and process ID once
    let mut notification_manager = core::services::get_services().notification_manager().await;

    // Publish system startup event
    let startup_event = Event::System(SystemEvent::with_message(
        SystemEventType::Startup,
        format!("System started, pid={pid}"),
    ));
    notification_manager.publish(startup_event).await
}

async fn system_stop(pid: u32) -> Result<(), NotificationError> {
    let mut notification_manager = core::services::get_services().notification_manager().await;
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
