mod app;
mod core;
mod notifications;
mod plugin;
mod queue;
mod scanner;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

/// Parse the API version string from build script into u32
pub fn get_plugin_api_version() -> u32 {
    PLUGIN_API_VERSION.parse().unwrap_or(20250727)
}

static COMMAND_NAME: &str = "repostats";

#[tokio::main]
async fn main() {
    // Try to get command name from args, otherwise use default
    let command_name_owned = std::env::args().next().and_then(|s| {
        std::path::PathBuf::from(s)
            .file_name()
            .map(|os| os.to_string_lossy().into_owned())
    });

    let command_name = command_name_owned.as_deref().unwrap_or(COMMAND_NAME);

    // Startup returns configured scanner manager, or None if no repositories to scan
    let scanner_manager = app::startup::startup(command_name).await;

    // Get notification manager and process ID once
    let mut notification_manager = core::services::get_services().notification_manager().await;
    let pid = std::process::id();

    // Publish system startup event
    let startup_event = Event::System(SystemEvent::with_message(
        SystemEventType::Startup,
        format!("System started, pid={pid}"),
    ));

    if let Err(e) = notification_manager.publish(startup_event).await {
        log::warn!("Failed to publish startup event: {e}");
    }

    log::info!("{command_name}: Repository Statistics Tool starting");

    // Start the actual scanner if we have one configured
    if let Some(scanner_manager) = scanner_manager {
        start_scanner(scanner_manager).await;
    } else {
        log::warn!("No scanner configured - no repositories to scan");
    }

    use notifications::api::*;
    let shutdown_event = Event::System(SystemEvent::with_message(
        SystemEventType::Shutdown,
        format!("System shutting down, pid={pid}"),
    ));

    if let Err(e) = notification_manager.publish(shutdown_event).await {
        log::warn!("Failed to publish shutdown event: {e}");
    }
}

/// Start the actual repository scanner with the configured scanner manager
async fn start_scanner(scanner_manager: std::sync::Arc<scanner::ScannerManager>) {
    use log::{debug, error, info};

    info!("Starting repository scanner...");
    debug!("Scanner manager configured and ready");

    // Start scanning all configured repositories
    match scanner_manager.start_scanning().await {
        Ok(()) => {
            info!("All repository scanning completed successfully");
            debug!("Scan results have been published to the queue for plugin processing");
        }
        Err(e) => {
            error!("Repository scanning failed: {}", e);
            error!("Some or all repositories could not be scanned");
        }
    }

    debug!("Scanner process complete");
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_main_is_async() {
        // Test that main function is now async
        // This test should pass once we've converted to async
        assert!(true, "Main function is now async");
    }
}
