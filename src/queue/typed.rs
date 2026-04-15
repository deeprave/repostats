//! Typed queue consumers for type-safe message handling
//!
//! This module provides typed wrappers around the generic queue system,
//! allowing plugins to work directly with strongly-typed messages
//! instead of manually deserializing from generic Message wrappers.

use crate::queue::api::{Message, QueueConsumer, QueueResult};
use crate::queue::message::MessageHeader;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;

/// A typed queue consumer that automatically deserializes messages to a specific type
///
/// This wrapper eliminates manual deserialization and provides compile-time type safety
/// for plugin message handling.
///
/// # Type Parameters
/// * `T` - The message type to deserialize to (must implement `DeserializeOwned`)
///
/// # Example
/// ```rust,no_run
/// # use repostats::scanner::api::ScanMessage;
/// # use repostats::queue::api::{QueueConsumer, TypedQueueConsumer};
/// # fn example(base_consumer: QueueConsumer) -> Result<(), Box<dyn std::error::Error>> {
/// let typed_consumer: TypedQueueConsumer<ScanMessage> =
///     TypedQueueConsumer::new(base_consumer);
///
/// // Direct typed message reading - no manual deserialization needed!
/// match typed_consumer.read_with_header()? {
///     Some(typed_message) => {
///         let scan_message = typed_message.content;
///         println!("Received scan message: {:?}", scan_message);
///     }
///     None => println!("No messages available"),
/// }
/// # Ok(())
/// # }
/// ```
pub struct TypedQueueConsumer<T> {
    inner: QueueConsumer,
    _phantom: PhantomData<T>,
}

impl<T> TypedQueueConsumer<T>
where
    T: DeserializeOwned,
{
    /// Create a new typed consumer wrapping a base consumer
    pub fn new(inner: QueueConsumer) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }

    /// Get the underlying message header information along with typed content
    ///
    /// This provides access to metadata like sequence number, producer ID, etc.
    /// while still getting the strongly-typed message content.
    pub fn read_with_header(&self) -> QueueResult<Option<TypedMessage<T>>> {
        match self.inner.read()? {
            Some(message) => {
                let typed_content = self.deserialize_message(&message)?;
                let typed_message = TypedMessage {
                    header: message.header.clone(),
                    content: typed_content,
                };
                Ok(Some(typed_message))
            }
            None => Ok(None),
        }
    }

    /// Deserialize an Arc<Message> to the target type
    fn deserialize_message(&self, message: &Arc<Message>) -> QueueResult<T> {
        serde_json::from_str(&message.data).map_err(|e| {
            let data_preview = if message.data.len() > 100 {
                let truncated_bytes = &message.data.as_bytes()[..100.min(message.data.len())];
                format!("{}...", String::from_utf8_lossy(truncated_bytes))
            } else {
                message.data.clone()
            };

            crate::queue::error::QueueError::DeserializationError {
                message: format!(
                    "Failed to deserialize message to {}: {} | sequence: {}, type: '{}', producer: '{}' | data_length: {}, data_preview: '{}'",
                    std::any::type_name::<T>(),
                    e,
                    message.header.sequence,
                    message.header.message_type,
                    message.header.producer_id,
                    message.data.len(),
                    data_preview
                ),
            }
        })
    }
}

/// A typed message containing both header metadata and strongly-typed content
#[derive(Debug, Clone)]
pub struct TypedMessage<T> {
    /// Message header with metadata (sequence, producer, timestamp, etc.)
    pub header: MessageHeader,
    /// Strongly-typed message content
    pub content: T,
}

// Tests are located in src/queue/tests/typed.rs
