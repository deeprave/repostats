//! Scanner Task Queue Operations
//!
//! Queue-related operations including publishers and message publishing.

use crate::core::services::get_services;
use crate::queue::{Message, QueuePublisher};
use crate::scanner::error::{ScanError, ScanResult};
use crate::scanner::types::ScanMessage;
use serde_json;

use super::core::ScannerTask;

impl ScannerTask {
    /// Create a queue publisher for this scanner task
    pub async fn create_queue_publisher(&self) -> ScanResult<QueuePublisher> {
        // Get the queue manager from services
        let services = get_services();
        let queue_manager = services.queue_manager();

        // Create a publisher using the scanner ID as the producer ID
        let publisher = queue_manager
            .create_publisher(self.scanner_id().to_string())
            .map_err(|e| ScanError::Configuration {
                message: format!("Failed to create queue publisher: {}", e),
            })?;

        Ok(publisher)
    }

    /// Publish scan messages to the queue
    pub async fn publish_messages(&self, messages: Vec<ScanMessage>) -> ScanResult<()> {
        // Create a queue publisher
        let publisher = self.create_queue_publisher().await?;
        let scanner_id = self.scanner_id().to_string();

        // Use spawn_blocking to prevent blocking the async executor
        tokio::task::spawn_blocking(move || {
            // Publish each message to the queue
            for scan_message in messages {
                // Serialize the scan message to JSON
                let json_data =
                    serde_json::to_string(&scan_message).map_err(|e| ScanError::Io {
                        message: format!("Failed to serialize message: {}", e),
                    })?;

                // Determine message type based on scan message variant
                let message_type = match &scan_message {
                    ScanMessage::RepositoryData { .. } => "repository_data",
                    ScanMessage::ScanStarted { .. } => "scan_started",
                    ScanMessage::CommitData { .. } => "commit_data",
                    ScanMessage::FileChange { .. } => "file_change",
                    ScanMessage::ScanCompleted { .. } => "scan_completed",
                    ScanMessage::ScanError { .. } => "scan_error",
                };

                // Create a queue message
                let queue_message =
                    Message::new(scanner_id.clone(), message_type.to_string(), json_data);

                // Publish to the queue (synchronous operation in blocking thread)
                publisher
                    .publish(queue_message)
                    .map_err(|e| ScanError::Io {
                        message: format!("Failed to publish message to queue: {}", e),
                    })?;
            }

            Ok::<(), ScanError>(())
        })
        .await
        .map_err(|e| ScanError::Io {
            message: format!("Failed to execute queue publishing task: {}", e),
        })?
    }
}
