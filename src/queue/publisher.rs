//! Queue Publisher for sending messages
//!
//! Publishers send messages to the global queue where they become available
//! to all registered consumers. Each publisher is identified by a unique
//! producer_id that is included in message headers.

use crate::queue::error::{QueueError, QueueResult};
use crate::queue::manager::QueueManager;
use crate::queue::message::Message;
use std::sync::Weak;

/// Publisher handle for sending messages to the queue
///
/// The QueuePublisher provides a lightweight handle for publishing messages
/// to the global queue. Messages are assigned monotonic sequence numbers
/// to ensure ordering guarantees across all publishers.
///
/// # Memory Management
///
/// The publisher automatically checks for memory pressure after each
/// publish operation and triggers garbage collection if needed.
///
/// # Example
///
/// ```rust,no_run
/// # use repostats::queue::api::{QueueManager, Message};
/// # use std::sync::Arc;
/// # fn example(manager: Arc<QueueManager>) -> Result<(), Box<dyn std::error::Error>> {
/// let publisher = manager.create_publisher("my-service".to_string())?;
///
/// // Create and publish a message
/// let message = Message::new(
///     "my-service".to_string(),
///     "user_action".to_string(),
///     "User clicked button".to_string()
/// );
///
/// let sequence = publisher.publish(message)?;
/// println!("Published message with sequence: {}", sequence);
/// # Ok(())
/// # }
/// ```
pub struct QueuePublisher {
    producer_id: String,
    manager: Weak<QueueManager>,
}

impl QueuePublisher {
    pub(crate) fn new(producer_id: String, manager: Weak<QueueManager>) -> Self {
        Self {
            producer_id,
            manager,
        }
    }

    pub fn producer_id(&self) -> &str {
        &self.producer_id
    }

    /// Publish a message to the global queue
    pub fn publish(&self, message: Message) -> QueueResult<u64> {
        // Get strong reference to manager
        let manager = self
            .manager
            .upgrade()
            .ok_or_else(|| QueueError::OperationFailed {
                message: "QueueManager no longer exists".to_string(),
            })?;

        // Get the single global queue (producer_id is just metadata in the message)
        let queue = manager.get_global_queue()?;

        // Publish to the global queue
        let sequence = queue.publish(message)?;

        // Check memory pressure after publishing and trigger garbage collection if needed
        let _memory_pressure_handled = manager.check_memory_pressure()?;

        Ok(sequence)
    }
}
