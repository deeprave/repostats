//! QueueManager - Central coordination for multiconsumer queue
//!
//! The QueueManager serves as the central coordination point for all queue operations.
//! It manages a single global queue that all producers publish to and all consumers
//! read from, with each consumer maintaining its own independent position.

use crate::notifications::api::{Event, QueueEvent, QueueEventType};
use crate::queue::consumer::QueueConsumer;
use crate::queue::error::QueueResult;
use crate::queue::internal::MultiConsumerQueue;
use crate::queue::publisher::QueuePublisher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Central queue manager providing producer/consumer coordination
///
/// The QueueManager is responsible for:
/// - Creating and managing publishers and consumers
/// - Maintaining the global message queue
/// - Memory management and backpressure
/// - Consumer lag monitoring and cleanup
/// - Integration with the event notification system
///
/// # Thread Safety
///
/// The QueueManager is fully thread-safe and can be shared across threads
/// using `Arc<QueueManager>`. All operations are atomic or protected by
/// appropriate synchronisation primitives.
///
/// # Example
///
/// ```rust,no_run
/// use repostats::queue::api::QueueManager;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let manager = QueueManager::create().await;
///
/// // Create consumers and publishers for message passing
/// let consumer = manager.create_consumer("scan_messages".to_string())?;
/// let publisher = manager.create_publisher("scan_messages".to_string())?;
///
/// # Ok(())
/// # }
/// ```
pub struct QueueManager {
    next_consumer_id: AtomicU64,
    /// Single global queue for all messages from all producers
    global_queue: Arc<MultiConsumerQueue>,
    #[cfg(test)]
    notification_manager:
        Option<Arc<tokio::sync::Mutex<crate::notifications::manager::AsyncNotificationManager>>>,
}

impl QueueManager {
    const GLOBAL_QUEUE_ID: &'static str = "global";
    const EVENT_PUBLISH_TIMEOUT: Duration = Duration::from_millis(100);

    pub fn new() -> Self {
        Self {
            next_consumer_id: AtomicU64::new(0),
            global_queue: Arc::new(MultiConsumerQueue::new(
                Self::GLOBAL_QUEUE_ID.to_string(), // Single global queue name
                10000,                             // Default queue size
            )),
            #[cfg(test)]
            notification_manager: None,
        }
    }
    #[cfg(test)]
    pub fn new_with_notification_manager(
        manager: Arc<tokio::sync::Mutex<crate::notifications::manager::AsyncNotificationManager>>,
    ) -> Self {
        Self {
            next_consumer_id: AtomicU64::new(0),
            global_queue: Arc::new(MultiConsumerQueue::new(
                Self::GLOBAL_QUEUE_ID.to_string(), // Single global queue name
                10000,                             // Default queue size
            )),
            notification_manager: Some(manager),
        }
    }

    /// Publish a queue lifecycle event with timeout protection
    ///
    /// This method publishes lifecycle events with a timeout to prevent deadlocks.
    /// Event publishing failures are logged but do not prevent queue operations.
    async fn publish_lifecycle_event(
        &self,
        event_type: QueueEventType,
        context: &str,
    ) -> Result<(), crate::notifications::error::NotificationError> {
        let event_type_str = match event_type {
            QueueEventType::Started => "Started",
            QueueEventType::Shutdown => "Shutdown",
            _ => "Other",
        };
        #[cfg(test)]
        let mut notification_manager = if let Some(ref nm) = self.notification_manager {
            nm.lock().await
        } else {
            crate::notifications::api::get_notification_service().await
        };
        #[cfg(not(test))]
        let mut notification_manager = crate::notifications::api::get_notification_service().await;
        let event = Event::Queue(QueueEvent::new(
            event_type,
            Self::GLOBAL_QUEUE_ID.to_string(),
        ));
        let publish_result = timeout(
            Self::EVENT_PUBLISH_TIMEOUT,
            notification_manager.publish(event),
        )
        .await;

        match publish_result {
            Ok(Ok(_)) => {
                log::trace!(
                    "Queue lifecycle event published successfully: {} for {}",
                    event_type_str,
                    context
                );
                Ok(())
            }
            Ok(Err(e)) => {
                log::error!(
                    "Failed to publish queue lifecycle event ({}) for {} - notification system may be degraded: {:?}",
                    event_type_str,
                    context,
                    e
                );
                Err(e)
            }
            Err(_) => {
                log::warn!(
                    "Timeout publishing queue lifecycle event ({}) for {} - continuing operation",
                    event_type_str,
                    context
                );
                Err(
                    crate::notifications::error::NotificationError::ChannelClosed(format!(
                        "Queue lifecycle event publish timeout: {} for {}",
                        event_type_str, context
                    )),
                )
            }
        }
    }

    /// Create a QueueManager and publish Started lifecycle event
    ///
    /// This method creates a new QueueManager instance and publishes
    /// a Started lifecycle event to the notification system, allowing
    /// other components to react to queue system initialization.
    ///
    /// Event publishing is protected by timeout and failures are logged
    /// but do not prevent queue system startup.
    ///
    /// # Returns
    ///
    /// Returns an `Arc<QueueManager>` for shared ownership across threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use repostats::queue::api::QueueManager;
    /// # async fn example() {
    /// let manager = QueueManager::create().await;
    /// // Queue system is now ready for use
    /// # }
    /// ```
    pub async fn create() -> Arc<Self> {
        let manager = Arc::new(Self::new());
        // Publish Started event with timeout protection
        let _ = manager
            .publish_lifecycle_event(QueueEventType::Started, "queue creation")
            .await;
        manager
    }
    #[cfg(test)]
    pub async fn create_with_notification_manager(
        manager: Arc<tokio::sync::Mutex<crate::notifications::manager::AsyncNotificationManager>>,
    ) -> Arc<Self> {
        let manager_arc = Arc::new(Self::new_with_notification_manager(manager.clone()));
        // Publish Started event with timeout protection
        let _ = manager_arc
            .publish_lifecycle_event(
                QueueEventType::Started,
                "QueueManager::create_with_notification_manager",
            )
            .await;
        manager_arc
    }

    /// Create a publisher for a specific producer_id (publishes to global queue)
    pub fn create_publisher(self: &Arc<Self>, producer_id: String) -> QueueResult<QueuePublisher> {
        Ok(QueuePublisher::new(producer_id, Arc::downgrade(self)))
    }

    /// Create a consumer for a plugin (reads from global queue)
    pub fn create_consumer(self: &Arc<Self>, plugin_name: String) -> QueueResult<QueueConsumer> {
        let consumer_id = self.next_consumer_id.fetch_add(1, Ordering::SeqCst);
        QueueConsumer::new(
            format!("consumer-{}", consumer_id),
            plugin_name,
            Arc::downgrade(self),
            consumer_id,
        )
    }

    /// Get the single global queue
    pub fn get_global_queue(&self) -> QueueResult<Arc<MultiConsumerQueue>> {
        Ok(Arc::clone(&self.global_queue))
    }

    /// Legacy method for compatibility - always returns the global queue
    pub fn get_queue(&self, _producer_id: &str) -> QueueResult<Arc<MultiConsumerQueue>> {
        Ok(Arc::clone(&self.global_queue))
    }

    /// Get number of queues managed (always 1 for single global queue)
    pub fn queue_count(&self) -> usize {
        1
    }

    /// Get total number of messages in the global queue
    pub fn total_message_count(&self) -> QueueResult<usize> {
        self.global_queue.size()
    }

    /// Get list of producer IDs that have published messages (not applicable for global queue)
    /// This method is kept for compatibility but doesn't make much sense with single queue
    pub fn producer_ids(&self) -> Vec<String> {
        vec!["global".to_string()] // Just return the global queue identifier
    }

    /// Get the number of active consumers
    pub fn active_consumer_count(&self) -> QueueResult<usize> {
        let consumer_ids = self.global_queue.consumer_ids()?;
        Ok(consumer_ids.len())
    }

    /// Shutdown the QueueManager and publish Shutdown lifecycle event
    ///
    /// This method performs a graceful shutdown of the queue manager,
    /// publishing a Shutdown lifecycle event to notify other components
    /// that the queue system is shutting down.
    ///
    /// Event publishing is protected by timeout and failures are logged
    /// but do not prevent queue system shutdown.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use repostats::queue::api::QueueManager;
    /// # async fn example() {
    /// let manager = QueueManager::create().await;
    /// // Use the queue manager...
    /// manager.shutdown().await;
    /// # }
    /// ```
    pub async fn shutdown(&self) {
        // Publish Shutdown event with timeout protection
        let _ = self
            .publish_lifecycle_event(QueueEventType::Shutdown, "queue shutdown")
            .await;
    }
}
