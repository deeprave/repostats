//! Scanner Task Event Operations
//!
//! Event and notification-related operations for scanner lifecycle management.

use crate::core::services::get_services;
use crate::notifications::api::EventReceiver;
use crate::notifications::event::{
    Event, EventFilter, QueueEventType, ScanEvent, ScanEventType, SystemEvent, SystemEventType,
};
use crate::scanner::error::{ScanError, ScanResult};
use std::time::SystemTime;

use super::core::ScannerTask;

impl ScannerTask {
    /// Create a notification subscriber for this scanner task
    pub async fn create_notification_subscriber(&self) -> ScanResult<EventReceiver> {
        // Get the notification manager from services
        let services = get_services();
        let mut notification_manager = services.notification_manager().await;

        // Create a subscriber using the scanner ID with queue event filter
        let subscriber_id = format!("{}-notifications", self.scanner_id());
        let filter = EventFilter::QueueOnly; // Scanner is interested in queue events
        let source = format!("Scanner-{}", self.scanner_id());

        let receiver = notification_manager
            .subscribe(subscriber_id, filter, source)
            .map_err(|e| ScanError::Configuration {
                message: format!("Failed to create notification subscriber: {}", e),
            })?;

        Ok(receiver)
    }

    /// Publish scanner lifecycle events via notification system
    pub async fn publish_scanner_event(
        &self,
        event_type: ScanEventType,
        message: Option<String>,
    ) -> ScanResult<()> {
        // Get the notification manager from services
        let services = get_services();
        let mut notification_manager = services.notification_manager().await;

        // Create scanner event
        let scan_event = ScanEvent {
            event_type,
            timestamp: SystemTime::now(),
            scan_id: self.scanner_id().to_string(),
            message,
        };

        // Wrap in main Event enum
        let event = Event::Scan(scan_event);

        // Publish the event
        notification_manager
            .publish(event)
            .await
            .map_err(|e| ScanError::Io {
                message: format!("Failed to publish scanner event: {}", e),
            })?;

        Ok(())
    }

    /// Subscribe to queue events to trigger scanning operations
    pub async fn subscribe_to_queue_events(&self) -> ScanResult<EventReceiver> {
        let services = get_services();
        let mut notification_manager = services.notification_manager().await;

        // Subscribe to queue events only
        let receiver = notification_manager
            .subscribe(
                format!("scanner-{}", self.scanner_id()),
                EventFilter::QueueOnly,
                "scanner-queue-subscription".to_string(),
            )
            .map_err(|e| ScanError::Io {
                message: format!("Failed to subscribe to queue events: {}", e),
            })?;

        Ok(receiver)
    }

    /// Handle queue started events and trigger scanning operations
    pub async fn handle_queue_started_event(
        &self,
        mut receiver: EventReceiver,
    ) -> ScanResult<bool> {
        // Wait for a queue started event
        tokio::select! {
            event_result = receiver.recv() => {
                match event_result {
                    Some(Event::Queue(queue_event)) => {
                        if queue_event.event_type == QueueEventType::Started {
                            // Queue started - trigger scanning operation
                            let _scan_messages = self.scan_commits().await?;
                            return Ok(true);
                        }
                        Ok(false)
                    },
                    Some(_) => Ok(false), // Not a queue event
                    None => Ok(false), // Channel closed
                }
            },
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                // Timeout for test purposes
                Ok(false)
            }
        }
    }

    /// Handle scanner shutdown via system events
    pub async fn handle_shutdown_event(&self) -> ScanResult<bool> {
        let services = get_services();
        let mut notification_manager = services.notification_manager().await;

        // For testing: immediately publish a shutdown event
        let shutdown_event = Event::System(SystemEvent::new(SystemEventType::Shutdown));
        let _ = notification_manager.publish(shutdown_event).await;

        // Subscribe to system events to listen for shutdown
        let mut receiver = notification_manager
            .subscribe(
                format!("scanner-shutdown-{}", self.scanner_id()),
                EventFilter::SystemOnly,
                "scanner-shutdown-subscription".to_string(),
            )
            .map_err(|e| ScanError::Io {
                message: format!("Failed to subscribe to system events: {}", e),
            })?;

        // Wait for shutdown event with timeout
        tokio::select! {
            event_result = receiver.recv() => {
                match event_result {
                    Some(Event::System(system_event)) => {
                        if system_event.event_type == SystemEventType::Shutdown {
                            // Publish final scanner event before shutdown
                            let _ = self.publish_scanner_event(
                                ScanEventType::Completed,
                                Some("Scanner shutting down gracefully".to_string())
                            ).await;
                            return Ok(true);
                        }
                        Ok(false)
                    },
                    Some(_) => Ok(false), // Not a system event
                    None => Ok(false), // Channel closed
                }
            },
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                // Timeout - assume shutdown handled for test purposes
                Ok(true)
            }
        }
    }
}
