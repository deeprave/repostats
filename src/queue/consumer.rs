//! Queue Consumer for processing Message objects
//!
//! Consumers read messages from the global queue, with each consumer maintaining
//! its own independent position. This allows multiple consumers to process the
//! same message stream at their own pace.

use crate::queue::{Message, QueueManager, QueueResult};
use std::sync::{Arc, Weak};

/// Consumer handle for reading messages from the queue
///
/// Each QueueConsumer maintains an independent position in the message
/// stream, allowing it to read messages at its own pace without affecting
/// other consumers. The consumer automatically registers with the queue
/// on creation and unregisters on drop.
///
/// # Example
///
/// ```rust,no_run
/// # use repostats::queue::{QueueManager, Message};
/// # use std::sync::Arc;
/// # fn example(manager: Arc<QueueManager>) -> Result<(), Box<dyn std::error::Error>> {
/// let consumer = manager.create_consumer("my-plugin".to_string())?;
///
/// // Read messages one at a time
/// while let Some(message) = consumer.read()? {
///     println!("Processing: {}", message.data);
/// }
///
/// // Or read in batches for better performance
/// let batch = consumer.read_batch(10)?;
/// for message in batch {
///     println!("Batch processing: {}", message.data);
/// }
/// # Ok(())
/// # }
/// ```
pub struct QueueConsumer {
    consumer_id: String,
    plugin_name: String,
    manager: Weak<QueueManager>,
    internal_consumer_id: u64,
}

impl QueueConsumer {
    pub(crate) fn new(
        consumer_id: String,
        plugin_name: String,
        manager: Weak<QueueManager>,
        internal_consumer_id: u64,
    ) -> QueueResult<Self> {
        let consumer = Self {
            consumer_id,
            plugin_name,
            manager: manager.clone(),
            internal_consumer_id,
        };

        // Register with the global queue
        if let Some(mgr) = manager.upgrade() {
            let queue = mgr.get_global_queue()?;
            queue.register_consumer(internal_consumer_id)?;
        }

        Ok(consumer)
    }

    pub fn consumer_id(&self) -> &str {
        &self.consumer_id
    }

    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }

    /// Get the internal consumer ID (used for queue position tracking)
    pub fn internal_consumer_id(&self) -> u64 {
        self.internal_consumer_id
    }

    /// Read the next available message from the global queue
    pub fn read(&self) -> QueueResult<Option<Arc<Message>>> {
        // Get strong reference to manager
        let manager =
            self.manager
                .upgrade()
                .ok_or_else(|| crate::queue::QueueError::OperationFailed {
                    message: "QueueManager no longer exists".to_string(),
                })?;

        // Get the global queue and read next message
        let queue = manager.get_global_queue()?;
        queue.read_next(self.internal_consumer_id)
    }

    /// Read a batch of messages from the global queue for improved performance
    pub fn read_batch(&self, batch_size: usize) -> QueueResult<Vec<Arc<Message>>> {
        let mut batch = Vec::with_capacity(batch_size);

        // Read up to batch_size messages
        for _ in 0..batch_size {
            match self.read()? {
                Some(message) => batch.push(message),
                None => break, // No more messages available
            }
        }

        Ok(batch)
    }

    /// Acknowledge a batch of messages (currently a no-op for compatibility)
    /// In a full implementation, this would be used for at-least-once delivery guarantees
    pub fn acknowledge_batch(&self, _messages: &[Arc<Message>]) -> QueueResult<usize> {
        // For now, just return the count of messages "acknowledged"
        // In a production system, this would update consumer commits/offsets
        Ok(_messages.len())
    }
}

impl Drop for QueueConsumer {
    fn drop(&mut self) {
        // Unregister from queue when consumer is dropped
        if let Some(manager) = self.manager.upgrade() {
            if let Ok(queue) = manager.get_global_queue() {
                let _ = queue.unregister_consumer(self.internal_consumer_id);
            }
        }
    }
}
