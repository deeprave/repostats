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
pub use crate::queue::message::Message;
#[allow(unused_imports)]
pub use crate::queue::traits::GroupedMessage;
#[allow(unused_imports)]
pub use crate::queue::typed::TypedQueueConsumer;

// Internal queue implementation (may be needed by some components)

// Error handling
pub use crate::queue::error::{QueueError, QueueResult};

// Type definitions and statistics

/// Global queue service instance
static QUEUE_SERVICE: LazyLock<Arc<QueueManager>> = LazyLock::new(|| {
    log::trace!("Initializing queue service");
    Arc::new(QueueManager::new())
});

/// Stable queue facade for interacting with the global queue service.
#[derive(Clone, Copy, Debug, Default)]
pub struct QueueService;

/// Access the stable queue facade.
pub fn queue_service() -> QueueService {
    QueueService
}

impl QueueService {
    /// Create a publisher on the global queue service.
    pub fn create_publisher(self, producer_id: String) -> QueueResult<QueuePublisher> {
        QUEUE_SERVICE.create_publisher(producer_id)
    }

    /// Create a consumer on the global queue service.
    pub fn create_consumer(self, plugin_name: String) -> QueueResult<QueueConsumer> {
        QUEUE_SERVICE.create_consumer(plugin_name)
    }

    pub(crate) fn manager(self) -> Arc<QueueManager> {
        Arc::clone(&QUEUE_SERVICE)
    }
}

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
    queue_service().manager()
}
