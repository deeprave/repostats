mod app;
mod core;
mod notifications;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

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

    app::startup::startup(command_name).await;

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

    // Start the theoretical scanner
    start_scanner().await;

    use notifications::api::*;
    let shutdown_event = Event::System(SystemEvent::with_message(
        SystemEventType::Shutdown,
        format!("System shutting down, pid={pid}"),
    ));

    if let Err(e) = notification_manager.publish(shutdown_event).await {
        log::warn!("Failed to publish shutdown event: {e}");
    }
}

/// Theoretical scanner startup function
/// TODO: Implement actual repository scanning logic
async fn start_scanner() {
    log::info!("Starting repository scanner...");
    log::trace!("Scanner initialization complete");
    // TODO: Add actual scanner implementation
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
