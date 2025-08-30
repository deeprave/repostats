//! Public API for the queue system
//!
//! This module provides the complete public API for the multiconsumer queue system.
//! External modules should import from here rather than directly from internal modules.
//! See module documentation for complete usage examples and architecture details.

// Core queue components
pub use crate::queue::consumer::QueueConsumer;
pub use crate::queue::manager::QueueManager;
pub use crate::queue::publisher::QueuePublisher;

// Message types and utilities
pub use crate::queue::message::{Message, MessageHeader};

// Internal queue implementation (may be needed by some components)
pub use crate::queue::internal::MultiConsumerQueue;

// Error handling
pub use crate::queue::error::{QueueError, QueueResult};

// Type definitions and statistics
pub use crate::queue::types::{LagStats, MemoryStats, StaleConsumerInfo};

// Traits
pub use crate::queue::traits::GroupedMessage;
