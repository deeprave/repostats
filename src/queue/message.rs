//! Message Types for Generic Producer/Consumer Queue System
//!
//! This module defines the core message structures used throughout the queue system.
//! Messages are designed to be generic and extensible, supporting various use cases
//! through the optional GroupedMessage trait.

use crate::queue::traits::GroupedMessage;
use std::time::SystemTime;

/// Header information for all messages in the queue
///
/// The MessageHeader contains metadata that is automatically populated
/// when messages are published to the queue.
#[derive(Debug, Clone)]
pub struct MessageHeader {
    /// Monotonic sequence number assigned by the queue
    pub sequence: u64,
    /// Timestamp when the message was created
    pub timestamp: SystemTime,
    /// Identifier of the producer that created this message
    pub producer_id: String,
    /// Application-defined message type for routing/filtering
    pub message_type: String,
}

/// Generic message structure for queue communication
///
/// Messages consist of a header with metadata and a string data payload.
/// The design is intentionally simple to maximize compatibility across
/// different components and use cases.
///
/// # Example
///
/// ```rust
/// use repostats::queue::Message;
///
/// let message = Message::new(
///     "file-scanner".to_string(),
///     "file_discovered".to_string(),
///     "/path/to/file.rs".to_string()
/// );
/// ```
#[derive(Debug, Clone)]
pub struct Message {
    /// Message metadata
    pub header: MessageHeader,
    /// Message payload (application-specific data)
    pub data: String,
}

impl Message {
    pub fn new(producer_id: String, message_type: String, data: String) -> Self {
        Self {
            header: MessageHeader {
                sequence: 0, // Will be set by queue
                timestamp: SystemTime::now(),
                producer_id,
                message_type,
            },
            data,
        }
    }
}

impl GroupedMessage for Message {
    /// Default implementation - no grouping
    fn group_id(&self) -> Option<String> {
        None
    }

    fn starts_group(&self) -> Option<(String, usize)> {
        None
    }

    fn completes_group(&self) -> Option<String> {
        None
    }
}
