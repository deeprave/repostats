//! Message Types for Generic Producer/Consumer Queue System
//!
//! This module defines the core message structures used throughout the queue system.
//! Messages are designed to be generic and extensible, supporting various use cases
//! through the optional GroupedMessage trait.

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

/// Trait for messages that support logical grouping
///
/// This trait allows messages to be grouped for batch processing or
/// coordinated event publishing. Groups are scoped by both producer_id
/// and group_id to prevent conflicts between different producers.
///
/// # Example
///
/// ```rust
/// use repostats::queue::{Message, GroupedMessage};
///
/// struct BatchMessage {
///     message: Message,
///     batch_id: String,
///     batch_size: Option<usize>,
///     is_last: bool,
/// }
///
/// impl GroupedMessage for BatchMessage {
///     fn group_id(&self) -> Option<String> {
///         Some(self.batch_id.clone())
///     }
///
///     fn starts_group(&self) -> Option<(String, usize)> {
///         self.batch_size.map(|size| (self.batch_id.clone(), size))
///     }
///
///     fn completes_group(&self) -> Option<String> {
///         if self.is_last { Some(self.batch_id.clone()) } else { None }
///     }
/// }
/// ```
pub trait GroupedMessage {
    /// Get the group identifier for this message
    ///
    /// Returns `Some(group_id)` if this message belongs to a group,
    /// or `None` for standalone messages.
    fn group_id(&self) -> Option<String>;

    /// Check if this message starts a new group
    ///
    /// Returns `Some((group_id, expected_count))` if this message
    /// starts a new group with a known message count. This enables
    /// precise batching without waiting for completion signals.
    fn starts_group(&self) -> Option<(String, usize)>;

    /// Check if this message completes a group
    ///
    /// Returns `Some(group_id)` if this message completes a group.
    /// This is a fallback mechanism for groups without known counts.
    fn completes_group(&self) -> Option<String>;
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
