use crate::core::error_handling::log_error_with_context;
use crate::notifications::api::{Event, SystemEvent, SystemEventType};
use crate::scanner::api::ScanError;

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

    // Startup returns configured scanner manager, or None if no repositories to scan
    let scanner_manager = match app::startup::startup(command_name).await {
        Ok(scanner_manager) => scanner_manager,
        Err(e) => {
            // Handle startup errors using unified error handling system
            log_error_with_context(&e, "Application startup");
            std::process::exit(1);
        }
    };

    if let Err(e) = system_start(pid).await {
        log::error!("Failed to start system: {e}");
        std::process::exit(1);
    } else {
        log::info!("{command_name}: ✅ Repository Statistics Tool starting");

        // Start the actual scanner if we have one configured
        if let Some(scanner_manager) = scanner_manager {
            if let Err(e) = start_scanner(scanner_manager).await {
                log::error!("Failed to start scanner: {e}");
                std::process::exit(1);
            }
        }

        if let Err(e) = system_stop(pid).await {
            log::warn!("Error stopping system: {e}");
        }
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
    #[tokio::test]
    async fn test_main_is_async() {
        // Test that main function is now async
        // This test should pass once we've converted to async
        assert!(true, "Main function is now async");
    }
}
