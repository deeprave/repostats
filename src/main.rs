use repostats::app;
use repostats::app::event_controller::EventController;
use repostats::core::error_handling::log_error_with_context;
use repostats::notifications::api::{
    notification_service, Event, NotificationError, SystemEvent, SystemEventType,
};
use repostats::plugin;
use repostats::scanner;
use repostats::scanner::api::ScanError;

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

    // Use EventController to guard the entire application execution with transparent coordination
    let result = EventController::guard(|| async {
        // Application startup
        let scanner_manager = match app::startup::startup(command_name).await {
            Ok(scanner_manager) => scanner_manager,
            Err(e) => {
                log_error_with_context(&e, "Application startup");
                std::process::exit(1);
            }
        };

        // System start
        if let Err(e) = system_start(pid).await {
            log::error!("Failed to start system: {e}");
            std::process::exit(1);
        }

        // Spawn spinner task - it will self-manage based on events and plugin state
        tokio::spawn(async move {
            // Spinner will be automatically coordinated by EventController on shutdown
            if let Err(e) = app::spinner::run_spinner(tokio::sync::broadcast::channel(1).1).await {
                log::debug!("Spinner task failed: {}", e);
            }
        });

        // Handle scanner execution if configured
        let final_result = if let Some(scanner_manager) = scanner_manager {
            log::info!("{command_name}: ✅ Repository Statistics Tool starting");
            run_scanner_simple(scanner_manager).await
        } else {
            // Early exit
            Ok(())
        };

        // EventController will handle coordinated shutdown - do NOT broadcast shutdown event here
        // The coordination will happen in EventController::guard after the application completes

        final_result
    })
    .await;

    if let Err(e) = result {
        log::error!("Application error: {e}");
        std::process::exit(1);
    }
}

/// Run scanner with simplified logic - EventController handles all coordination transparently
async fn run_scanner_simple(
    scanner_manager: std::sync::Arc<scanner::api::ScannerManager>,
) -> Result<(), ScanError> {
    // Initialize plugin manager
    {
        if let Err(e) = plugin::api::plugin_service().initialize().await {
            log::error!("Failed to initialize plugin manager: {}", e);
            std::process::exit(1);
        }
    }

    // Get opaque cleanup handle for coordinated shutdown
    let cleanup_handle = scanner_manager.clone().cleanup_handle();

    // Run scanner - EventController automatically handles:
    // - Signal coordination
    // - Plugin graceful shutdown
    // - Plugin completion waiting
    // - Timeout handling
    let result = start_scanner(scanner_manager).await;

    // Always cleanup scanner resources
    cleanup_handle.cleanup();

    // EventController will coordinate plugin shutdown automatically
    result
}

async fn system_start(pid: u32) -> Result<(), NotificationError> {
    // Publish system startup event
    let startup_event = Event::System(SystemEvent::with_message(
        SystemEventType::Startup,
        format!("System started, pid={pid}"),
    ));
    notification_service().publish(startup_event).await
}

/// Start the actual repository scanner with the configured scanner manager
async fn start_scanner(
    scanner_manager: std::sync::Arc<scanner::api::ScannerManager>,
) -> Result<(), ScanError> {
    use log::debug;

    // Start scanning all configured repositories
    let result = scanner_manager.start_scanning().await;
    match &result {
        Ok(()) => log::trace!("All repository scanning completed successfully"),
        Err(e) => debug!("Repository scanning failed: {e}"),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_main_is_async() {
        let result = start_scanner(scanner::api::ScannerManager::create().await).await;

        assert!(
            result.is_err(),
            "start_scanner should return an error when no repository scanners are configured"
        );

        let error = result.unwrap_err();
        assert_eq!(
            error.to_string(),
            "Repository error: No active repository scanner"
        );
    }

    #[tokio::test]
    async fn test_event_controller_integration() {
        // Test that EventController::guard pattern works with simple logic
        let result: Result<(), ScanError> = EventController::guard(|| async { Ok(()) }).await;

        assert!(
            result.is_ok(),
            "EventController::guard should work with simple logic"
        );
    }
}
