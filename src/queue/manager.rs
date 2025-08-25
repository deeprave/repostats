//! QueueManager - Central coordination for multiconsumer queue
//!
//! The QueueManager serves as the central coordination point for all queue operations.
//! It manages a single global queue that all producers publish to and all consumers
//! read from, with each consumer maintaining its own independent position.

use crate::core::services::get_services;
use crate::notifications::event::{Event, QueueEvent, QueueEventType};
use crate::queue::{
    LagStats, MemoryStats, MultiConsumerQueue, QueueConsumer, QueuePublisher, QueueResult,
    StaleConsumerInfo,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

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
/// use repostats::queue::QueueManager;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let manager = QueueManager::create().await;
///
/// // Set memory threshold for automatic cleanup
/// manager.set_memory_threshold_bytes(1_000_000)?;
///
/// // Monitor memory usage
/// let stats = manager.memory_stats();
/// println!("Messages: {}, Memory: {} bytes",
///          stats.total_messages, stats.total_bytes);
/// # Ok(())
/// # }
/// ```
pub struct QueueManager {
    next_consumer_id: AtomicU64,
    /// Single global queue for all messages from all producers
    global_queue: Arc<MultiConsumerQueue>,
    /// Memory threshold in bytes for automatic garbage collection
    memory_threshold_bytes: RwLock<Option<usize>>,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            next_consumer_id: AtomicU64::new(0),
            global_queue: Arc::new(MultiConsumerQueue::new(
                "global".to_string(), // Single global queue name
                10000,                // Default queue size
            )),
            memory_threshold_bytes: RwLock::new(None), // No automatic garbage collection by default
        }
    }

    /// Create a QueueManager and publish Started event
    ///
    /// This method creates a new QueueManager instance and publishes
    /// a Started lifecycle event to the notification system, allowing
    /// other components to react to queue system initialization.
    ///
    /// # Returns
    ///
    /// Returns an `Arc<QueueManager>` for shared ownership across threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn example() {
    /// let manager = QueueManager::create().await;
    /// // Queue system is now ready for use
    /// # }
    /// ```
    pub async fn create() -> Arc<Self> {
        let manager = Arc::new(Self::new());

        // Publish Started event
        let services = get_services();
        let mut notification_manager = services.notification_manager().await;
        let started_event = Event::Queue(QueueEvent::new(
            QueueEventType::Started,
            "global".to_string(),
        ));
        match notification_manager.publish(started_event).await {
            Ok(_) => println!("✓ Started event published successfully"),
            Err(e) => println!("✗ Failed to publish Started event: {:?}", e),
        }

        manager
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
    pub fn total_message_count(&self) -> usize {
        self.global_queue.size()
    }

    /// Get list of producer IDs that have published messages (not applicable for global queue)
    /// This method is kept for compatibility but doesn't make much sense with single queue
    pub fn producer_ids(&self) -> Vec<String> {
        vec!["global".to_string()] // Just return the global queue identifier
    }

    /// Get memory usage in bytes for the global queue
    pub fn memory_usage_bytes(&self) -> usize {
        self.global_queue.memory_stats().total_bytes
    }

    /// Get detailed memory statistics for the global queue
    pub fn memory_stats(&self) -> MemoryStats {
        self.global_queue.memory_stats()
    }

    /// Set memory threshold for automatic garbage collection
    pub fn set_memory_threshold_bytes(&self, threshold: usize) -> QueueResult<()> {
        let mut threshold_lock = self.memory_threshold_bytes.write().unwrap();
        *threshold_lock = Some(threshold);
        Ok(())
    }

    /// Manually trigger garbage collection
    pub fn collect_garbage(&self) -> QueueResult<usize> {
        self.global_queue.collect_garbage()
    }

    /// Check if memory pressure exists and trigger automatic garbage collection if needed
    pub fn check_memory_pressure(&self) -> QueueResult<bool> {
        let threshold = {
            let threshold_lock = self.memory_threshold_bytes.read().unwrap();
            match *threshold_lock {
                Some(threshold) => threshold,
                None => return Ok(false), // No threshold set, no pressure
            }
        };

        let current_usage = self.memory_usage_bytes();
        if current_usage > threshold {
            // Trigger automatic garbage collection
            let _collected = self.collect_garbage()?;
            Ok(true) // Memory pressure detected and handled
        } else {
            Ok(false) // No memory pressure
        }
    }

    /// Get lag for a specific consumer (messages behind current head)
    pub fn get_consumer_lag(&self, consumer: &QueueConsumer) -> QueueResult<usize> {
        let consumer_id = consumer.internal_consumer_id();
        let current_head = self.global_queue.head_sequence();

        match self.global_queue.consumer_position(consumer_id) {
            Some(consumer_position) => {
                // Lag is the difference between current head and consumer position
                // Consumer position is the "next sequence to read", so lag is head - position
                let lag = if current_head >= consumer_position {
                    (current_head - consumer_position) as usize
                } else {
                    0 // Consumer is ahead somehow, no lag
                };
                Ok(lag)
            }
            None => {
                // Consumer not registered, consider it at maximum lag
                Ok(current_head as usize)
            }
        }
    }

    /// Get lag statistics for all registered consumers
    pub fn get_lag_statistics(&self) -> QueueResult<LagStats> {
        let consumer_ids = self.global_queue.consumer_ids();
        let current_head = self.global_queue.head_sequence();

        if consumer_ids.is_empty() {
            return Ok(LagStats {
                total_consumers: 0,
                max_lag: 0,
                min_lag: 0,
                avg_lag: 0.0,
            });
        }

        let mut lags = Vec::new();

        for consumer_id in consumer_ids {
            if let Some(consumer_position) = self.global_queue.consumer_position(consumer_id) {
                let lag = if current_head >= consumer_position {
                    (current_head - consumer_position) as usize
                } else {
                    0
                };
                lags.push(lag);
            }
        }

        if lags.is_empty() {
            return Ok(LagStats {
                total_consumers: 0,
                max_lag: 0,
                min_lag: 0,
                avg_lag: 0.0,
            });
        }

        let max_lag = *lags.iter().max().unwrap();
        let min_lag = *lags.iter().min().unwrap();
        let avg_lag = lags.iter().sum::<usize>() as f64 / lags.len() as f64;

        Ok(LagStats {
            total_consumers: lags.len(),
            max_lag,
            min_lag,
            avg_lag,
        })
    }

    /// Get the number of active consumers
    pub fn active_consumer_count(&self) -> QueueResult<usize> {
        let consumer_ids = self.global_queue.consumer_ids();
        Ok(consumer_ids.len())
    }

    /// Detect stale consumers based on lag threshold
    pub fn detect_stale_consumers(
        &self,
        _stale_threshold_seconds: u64,
    ) -> QueueResult<Vec<StaleConsumerInfo>> {
        let consumer_ids = self.global_queue.consumer_ids();
        let current_head = self.global_queue.head_sequence();
        let now = SystemTime::now();

        let mut stale_consumers = Vec::new();

        for consumer_id in consumer_ids {
            let consumer_position = self.global_queue.consumer_position(consumer_id);
            let last_read_time = self.global_queue.consumer_last_read_time(consumer_id);

            if let (Some(position), Some(last_time)) = (consumer_position, last_read_time) {
                let lag = if current_head >= position {
                    (current_head - position) as usize
                } else {
                    0
                };

                let seconds_since_last_read = match now.duration_since(last_time) {
                    Ok(duration) => duration.as_secs(),
                    Err(_) => 0, // Clock went backwards, consider as 0
                };

                // For now, consider any consumer with lag > 0 as potentially stale
                // In a more sophisticated implementation, we'd use the time threshold
                if lag > 0 {
                    stale_consumers.push(StaleConsumerInfo {
                        consumer_id,
                        lag,
                        seconds_since_last_read,
                    });
                }
            }
        }

        // Sort by lag (descending) to prioritise the most stale consumers
        stale_consumers.sort_by(|a, b| b.lag.cmp(&a.lag));

        Ok(stale_consumers)
    }

    /// Cleanup stale consumers with lag above threshold
    pub fn cleanup_stale_consumers(&self, lag_threshold: usize) -> QueueResult<usize> {
        let stale_consumers = self.detect_stale_consumers(0)?;
        let mut cleanup_count = 0;

        for stale_info in stale_consumers {
            if stale_info.lag > lag_threshold {
                // Remove the stale consumer from the queue
                let _ = self
                    .global_queue
                    .unregister_consumer(stale_info.consumer_id);
                cleanup_count += 1;
            }
        }

        Ok(cleanup_count)
    }
}
