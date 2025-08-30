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

// Internal modules - all access should go through api module
pub(crate) mod consumer;
pub(crate) mod error;
pub(crate) mod internal;
pub(crate) mod manager;
pub(crate) mod message;
pub(crate) mod publisher;
pub(crate) mod traits;
pub(crate) mod types;

// Public API module - the only public interface for the queue system
pub mod api;

#[cfg(test)]
mod tests;
