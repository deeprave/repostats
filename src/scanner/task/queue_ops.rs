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

    /// Publish scan messages to the queue with optimized batching while preserving order
    pub async fn publish_messages(&self, messages: Vec<ScanMessage>) -> ScanResult<()> {
        if messages.is_empty() {
            return Ok(());
        }

        let publisher = self.create_queue_publisher().await?;
        let scanner_id = self.scanner_id().to_string();

        // Use spawn_blocking to prevent blocking the async executor
        tokio::task::spawn_blocking(move || {
            // Pre-serialize all messages in batches for better CPU utilization
            let batch_size = 100;
            let mut queue_messages = Vec::with_capacity(messages.len());

            for chunk in messages.chunks(batch_size) {
                // Batch serialize messages (CPU intensive operation)
                for scan_message in chunk {
                    let json_data =
                        serde_json::to_string(&scan_message).map_err(|e| ScanError::Io {
                            message: format!("Failed to serialize message: {}", e),
                        })?;

                    let message_type = match &scan_message {
                        ScanMessage::RepositoryData { .. } => "repository_data",
                        ScanMessage::ScanStarted { .. } => "scan_started",
                        ScanMessage::CommitData { .. } => "commit_data",
                        ScanMessage::FileChange { .. } => "file_change",
                        ScanMessage::ScanCompleted { .. } => "scan_completed",
                        ScanMessage::ScanError { .. } => "scan_error",
                    };

                    queue_messages.push(Message::new(
                        scanner_id.clone(),
                        message_type.to_string(),
                        json_data,
                    ));
                }
            }

            // Publish all messages in order (I/O intensive operation)
            for queue_message in queue_messages {
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
