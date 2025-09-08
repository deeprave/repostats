//! Simple progress spinner for terminal feedback

use crate::notifications::api::{
    Event, EventFilter, ScanEvent, ScanEventType, SystemEvent, SystemEventType,
};
use std::io::Write;
use thiserror::Error;
use tokio::time::{interval, Duration};

/// Module-local result type for spinner operations
type Result<T> = std::result::Result<T, SpinnerError>;

/// Errors specific to the spinner module
#[derive(Debug, Error, Clone, PartialEq)]
pub enum SpinnerError {
    /// Subscribing to the notification service failed
    #[error("Failed to subscribe to events: {reason}")]
    SubscribeFailed { reason: String },
}

const BRAILLE_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Check if spinner should be displayed
pub fn should_show_spinner() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr()) && !log::log_enabled!(log::Level::Info)
}

/// Simple spinner struct
pub struct ProgressSpinner {
    frame_index: usize,
}

impl ProgressSpinner {
    pub fn new() -> Self {
        Self { frame_index: 0 }
    }

    pub fn tick(&mut self) {
        let frame = BRAILLE_FRAMES[self.frame_index];
        self.frame_index = (self.frame_index + 1) % BRAILLE_FRAMES.len();

        // Clear line and show spinner with message
        eprint!("\r{frame}");
        let _ = std::io::stderr().flush();
    }

    pub fn finish(&self) {
        eprint!("\r \r");
        let _ = std::io::stderr().flush();
    }
}

/// Run the spinner task
pub async fn run_spinner(mut shutdown_rx: tokio::sync::broadcast::Receiver<()>) -> Result<()> {
    if !should_show_spinner() {
        return Ok(());
    }

    // Subscribe to events
    let mut notification_manager = crate::notifications::api::get_notification_service().await;
    let mut event_receiver = notification_manager
        .subscribe(
            "progress-spinner".to_string(),
            EventFilter::All, // Will filter manually for ScanEvent::Progress and SystemEvent::Shutdown
            "main-spinner".to_string(),
        )
        .map_err(|e| SpinnerError::SubscribeFailed {
            reason: e.to_string(),
        })?;
    drop(notification_manager);

    let mut spinner = ProgressSpinner::new();
    let mut update_interval = interval(Duration::from_millis(100)); // 10Hz

    loop {
        tokio::select! {
            // Handle shutdown signal
            _shutdown_result = shutdown_rx.recv() => {
                spinner.finish();
                return Ok(());
            }

            // Handle events
            event = event_receiver.recv() => {
                match event {
                    Some(Event::Scan(ScanEvent { event_type: ScanEventType::Progress, .. })) => {
                        spinner.tick();
                    }
                    Some(Event::System(SystemEvent { event_type: SystemEventType::Shutdown, .. })) => {
                        spinner.finish();
                        return Ok(());
                    }
                    Some(_) => {} // Ignore other events
                    None => {
                        spinner.finish();
                        return Ok(());
                    }
                }
            }

            // Update spinner animation
            _ = update_interval.tick() => {
                spinner.tick();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::api::{
        get_notification_service, Event, ScanEvent, ScanEventType, SystemEvent, SystemEventType,
    };
    use tokio::time::{timeout, Duration};

    #[test]
    fn test_spinner_creation() {
        let spinner = ProgressSpinner::new();
        assert_eq!(spinner.frame_index, 0);
    }

    #[test]
    fn test_braille_frames_cycle() {
        let mut spinner = ProgressSpinner::new();

        // Test cycling through frames
        for i in 0..BRAILLE_FRAMES.len() * 2 {
            let expected_index = i % BRAILLE_FRAMES.len();
            assert_eq!(spinner.frame_index, expected_index);
            spinner.tick(); // This increments frame_index
        }
    }

    #[tokio::test]
    #[ignore = "Integration test that requires exclusive access to global notification service"]
    async fn test_spinner_lifecycle_integration() {
        // Don't clear subscribers to avoid race conditions with other tests
        let notification_manager = get_notification_service().await;

        // Create shutdown channel
        let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

        // Start spinner task before publishing events
        let spinner_task = tokio::spawn(async move { run_spinner(shutdown_rx).await });

        // Give the spinner a moment to subscribe
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Publish a progress event
        let mut notification_manager = get_notification_service().await;
        let progress_event = Event::Scan(ScanEvent::with_message(
            ScanEventType::Progress,
            "test-scanner".to_string(),
            "Test progress message".to_string(),
        ));
        notification_manager
            .publish(progress_event.clone())
            .await
            .unwrap();

        // Publish another progress event
        notification_manager.publish(progress_event).await.unwrap();

        // Give spinner time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Shutdown
        let shutdown_event = Event::System(SystemEvent::new(SystemEventType::Shutdown));
        notification_manager.publish(shutdown_event).await.unwrap();

        // Wait for spinner to finish
        let result = timeout(Duration::from_millis(500), spinner_task).await;
        assert!(result.is_ok(), "Spinner should complete within timeout");

        let spinner_result = result.unwrap().unwrap();
        assert!(
            spinner_result.is_ok(),
            "Spinner should complete successfully"
        );
    }

    #[tokio::test]
    async fn test_spinner_should_not_run_when_conditions_not_met() {
        // This test checks that spinner respects conditions
        if should_show_spinner() {
            // If conditions are met in test environment, skip this test
            return;
        }

        let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);

        // Spinner should return immediately if conditions not met
        let result = run_spinner(shutdown_rx).await;
        assert!(result.is_ok(), "Spinner should return Ok when disabled");
    }
}
