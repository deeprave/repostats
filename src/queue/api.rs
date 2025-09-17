//! Public API for the queue system
//!
//! This module provides the complete public API for the multiconsumer queue system.
//! External modules should import from here rather than directly from internal modules.
//! See module documentation for complete usage examples and architecture details.

use std::sync::{Arc, LazyLock};

// Core queue components
pub use crate::queue::consumer::QueueConsumer;
pub use crate::queue::manager::QueueManager;
pub use crate::queue::publisher::QueuePublisher;

// Message types and utilities
pub use crate::queue::message::{Message, MessageHeader}; // re-export header for test helpers

// Typed queue consumers for compile-time type safety
pub use crate::queue::typed::{TypedQueueConsumer, TypedQueueManagerExt};

// Internal queue implementation (may be needed by some components)

// Error handling
pub use crate::queue::error::{QueueError, QueueResult};

// Type definitions and statistics

// Traits
pub use crate::queue::traits::GroupedMessage;

/// Global queue service instance
static QUEUE_SERVICE: LazyLock<Arc<QueueManager>> = LazyLock::new(|| {
    log::trace!("Initializing queue service");
    Arc::new(QueueManager::new())
});

/// Access queue service
///
/// Returns a reference to the global queue service. This service manages
/// all message queues and provides methods to create publishers and consumers.
/// Each call returns the same shared instance.
///
/// # Examples
/// ```no_run
/// # use repostats::queue::api::get_queue_service;
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let queue_service = get_queue_service();
/// let publisher = queue_service.create_publisher("my-producer".to_string())?;
/// let consumer = queue_service.create_consumer("my-consumer".to_string())?;
/// # Ok(())
/// # }
/// ```
pub fn get_queue_service() -> Arc<QueueManager> {
    Arc::clone(&QUEUE_SERVICE)
}
