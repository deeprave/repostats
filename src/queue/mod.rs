//! Multiconsumer Queue Component
//!
//! A reusable multiconsumer queue implementation with sequence-based message ordering
//! and support for multiple concurrent producers and consumers. Based on sophisticated
//! design patterns from production queue systems.
//!
//! # Overview
//!
//! This module provides a generic producer/consumer queue system that enables
//! asynchronous communication between components. Key features include:
//!
//! - **Multiple Producers**: Any number of producers can publish messages concurrently
//! - **Multiple Consumers**: Each consumer maintains independent read position
//! - **Sequence Ordering**: Kafka-like monotonic sequence numbering ensures order
//! - **Memory Efficiency**: Arc-wrapped messages enable zero-copy sharing
//! - **Backpressure**: Automatic memory management with configurable thresholds
//! - **Event Integration**: Lifecycle events via the notification system
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
//! │  Producer A  │     │  Producer B  │     │  Producer C  │
//! └──────┬───────┘     └──────┬───────┘     └──────┬───────┘
//!        │ publish            │ publish            │ publish
//!        ▼                    ▼                    ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │              QueueManager (Global Singleton)            │
//! │  ┌─────────────────────────────────────────────────┐   │
//! │  │         MultiConsumerQueue (Global Queue)       │   │
//! │  │  ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐   │   │
//! │  │  │ 1 │ 2 │ 3 │ 4 │ 5 │ 6 │ 7 │ 8 │ 9 │...│   │   │
//! │  │  └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘   │   │
//! │  │     ▲       ▲           ▲                      │   │
//! │  │     │       │           │                      │   │
//! │  └─────┼───────┼───────────┼──────────────────────┘   │
//! └────────┼───────┼───────────┼────────────────────────────┘
//!          │ read  │ read      │ read
//! ┌────────┴──┐ ┌──┴──────┐ ┌──┴──────┐
//! │Consumer A │ │Consumer B│ │Consumer C│ (Independent positions)
//! └───────────┘ └──────────┘ └──────────┘
//! ```
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use repostats::queue::{QueueManager, Message};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create the queue manager
//! let manager = QueueManager::create().await;
//!
//! // Create a publisher
//! let publisher = manager.create_publisher("my-service".to_string())?;
//!
//! // Publish messages
//! let message = Message::new(
//!     "my-service".to_string(),
//!     "event_type".to_string(),
//!     "message data".to_string()
//! );
//! publisher.publish(message)?;
//!
//! // Create a consumer
//! let consumer = manager.create_consumer("my-plugin".to_string())?;
//!
//! // Read messages
//! while let Some(msg) = consumer.read()? {
//!     println!("Received: {}", msg.data);
//! }
//! # Ok(())
//! # }
//! ```

mod consumer;
mod error;
mod internal;
mod manager;
mod message;
mod publisher;

pub use consumer::QueueConsumer;
pub use error::QueueError;
pub use internal::MultiConsumerQueue;
pub use manager::QueueManager;
pub use message::{GroupedMessage, Message, MessageHeader};
pub use publisher::QueuePublisher;

/// Memory usage statistics for the queue system
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryStats {
    /// Total number of messages in the queue
    pub total_messages: usize,
    /// Total memory usage in bytes
    pub total_bytes: usize,
    /// Memory used by message data
    pub message_data_bytes: usize,
    /// Memory used by Arc and metadata overhead
    pub overhead_bytes: usize,
}

/// Consumer lag statistics for the queue system
#[derive(Debug, Clone, PartialEq)]
pub struct LagStats {
    /// Total number of active consumers
    pub total_consumers: usize,
    /// Maximum lag among all consumers
    pub max_lag: usize,
    /// Minimum lag among all consumers
    pub min_lag: usize,
    /// Average lag across all consumers
    pub avg_lag: f64,
}

/// Information about a stale consumer
#[derive(Debug, Clone)]
pub struct StaleConsumerInfo {
    /// Internal consumer ID
    pub consumer_id: u64,
    /// Current lag in messages
    pub lag: usize,
    /// Time since last read (seconds)
    pub seconds_since_last_read: u64,
}

pub type QueueResult<T> = Result<T, QueueError>;

#[cfg(test)]
mod concurrent_tests;
#[cfg(test)]
mod consumer_tests;
#[cfg(test)]
mod edge_case_tests;
#[cfg(test)]
mod grouping_tests;
#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod memory_tests;
#[cfg(test)]
mod publisher_tests;
#[cfg(test)]
mod tests;
