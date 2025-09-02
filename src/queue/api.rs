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
pub use crate::queue::message::Message;

// Typed queue consumers for compile-time type safety
pub use crate::queue::typed::{TypedMessage, TypedQueueConsumer, TypedQueueManagerExt};

// Internal queue implementation (may be needed by some components)

// Error handling
pub use crate::queue::error::{QueueError, QueueResult};

// Type definitions and statistics

// Traits
pub use crate::queue::traits::GroupedMessage;
