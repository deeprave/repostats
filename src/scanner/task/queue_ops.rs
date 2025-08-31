//! Scanner Task Queue Operations
//!
//! Queue-related operations including publishers and message publishing.

use crate::core::services::get_services;
use crate::queue::api::{Message, QueuePublisher};
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

    /// Create a queue message from a scan message (reusable helper)
    pub(crate) fn create_queue_message(&self, scan_message: &ScanMessage) -> ScanResult<Message> {
        let json_data = serde_json::to_string(scan_message).map_err(|e| ScanError::Io {
            message: format!(
                "Failed to serialize {} message for scanner '{}': {}",
                scan_message.message_type(),
                self.scanner_id(),
                e
            ),
        })?;

        let message_type = scan_message.message_type();

        Ok(Message::new(
            self.scanner_id().to_string(),
            message_type.to_string(),
            json_data,
        ))
    }

    /// Publish a single scan message to the queue
    pub async fn publish_message(&self, message: ScanMessage) -> ScanResult<()> {
        let publisher = self.create_queue_publisher().await?;
        let queue_message = self.create_queue_message(&message)?;

        publisher
            .publish(queue_message)
            .map_err(|e| ScanError::Io {
                message: format!("Failed to publish message to queue: {}", e),
            })?;

        Ok(())
    }

    /// Publish scan messages to the queue with streaming and backpressure control
    pub async fn publish_messages(&self, messages: Vec<ScanMessage>) -> ScanResult<()> {
        if messages.is_empty() {
            return Ok(());
        }

        let publisher = self.create_queue_publisher().await?;
        let scanner_id = self.scanner_id().to_string();

        // Process in smaller chunks with async yields to prevent memory buildup
        const CHUNK_SIZE: usize = 50;
        for chunk in messages.chunks(CHUNK_SIZE) {
            // Serialize chunk of messages
            let serialized_messages: Result<Vec<_>, _> = chunk
                .iter()
                .map(|scan_message| {
                    let json_data =
                        serde_json::to_string(scan_message).map_err(|e| ScanError::Io {
                            message: format!(
                                "Failed to serialize {} message for scanner '{}': {}",
                                scan_message.message_type(),
                                scanner_id,
                                e
                            ),
                        })?;

                    Ok(Message::new(
                        scanner_id.clone(),
                        scan_message.message_type().to_string(),
                        json_data,
                    ))
                })
                .collect();

            let serialized_messages = serialized_messages?;

            // Publish chunk of messages
            for queue_message in serialized_messages {
                publisher
                    .publish(queue_message)
                    .map_err(|e| ScanError::Io {
                        message: format!(
                            "Failed to publish message from scanner '{}': {}",
                            scanner_id, e
                        ),
                    })?;
            }

            // Yield control to allow other tasks to run and prevent blocking the executor
            tokio::task::yield_now().await;
        }

        Ok(())
    }
}
