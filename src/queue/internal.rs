//! Internal MultiConsumerQueue implementation with sequence-based ordering
//!
//! This module provides the core queue functionality with:
//! - Sequence-based message ordering (Kafka-like)
//! - Arc-wrapped messages for zero-copy sharing between consumers
//! - Per-consumer position tracking
//! - Memory management and garbage collection

use crate::queue::error::{QueueError, QueueResult};
use crate::queue::message::{Message, MessageHeader};
use crate::queue::types::MemoryStats;
use std::collections::{HashMap, VecDeque};
use std::mem;
use std::sync::{Arc, RwLock};

/// Internal queue entry with sequence number and Arc-wrapped message
#[derive(Debug, Clone)]
struct QueueEntry {
    sequence: u64,
    message: Arc<Message>,
}

/// Consumer position tracking
#[derive(Debug, Clone)]
struct ConsumerPosition {
    current_sequence: u64,
    last_read_timestamp: std::time::SystemTime,
}

/// MultiConsumerQueue provides sequence-based message ordering
/// with independent consumer position tracking
#[derive(Debug)]
pub struct MultiConsumerQueue {
    /// Monotonic sequence counter for message ordering
    next_sequence: RwLock<u64>,

    /// Internal message buffer with sequence ordering
    messages: RwLock<VecDeque<QueueEntry>>,

    /// Per-consumer position tracking
    consumer_positions: RwLock<HashMap<u64, ConsumerPosition>>,

    /// Maximum queue size before triggering memory management
    max_size: usize,

    /// Queue identifier
    queue_id: String,
}

impl MultiConsumerQueue {
    /// Create new MultiConsumerQueue with a specific identifier
    pub fn new(queue_id: String, max_size: usize) -> Self {
        Self {
            next_sequence: RwLock::new(1), // Start from 1, following Kafka convention
            messages: RwLock::new(VecDeque::new()),
            consumer_positions: RwLock::new(HashMap::new()),
            max_size,
            queue_id,
        }
    }

    /// Get the queue identifier
    pub fn queue_id(&self) -> &str {
        &self.queue_id
    }

    /// Get current queue size (number of messages)
    pub fn size(&self) -> usize {
        self.messages.read().unwrap().len()
    }

    /// Get the current head sequence number (next message to be assigned)
    pub fn head_sequence(&self) -> u64 {
        *self.next_sequence.read().unwrap()
    }

    /// Get the minimum sequence across all consumers (for garbage collection)
    pub fn min_consumer_sequence(&self) -> Option<u64> {
        let positions = self.consumer_positions.read().unwrap();
        positions.values().map(|pos| pos.current_sequence).min()
    }

    /// Register a new consumer with this queue
    pub fn register_consumer(&self, consumer_id: u64) -> QueueResult<()> {
        let mut positions = self.consumer_positions.write().unwrap();

        // Start consumer at current head sequence (won't receive historical messages)
        let current_sequence = *self.next_sequence.read().unwrap();

        positions.insert(
            consumer_id,
            ConsumerPosition {
                current_sequence,
                last_read_timestamp: std::time::SystemTime::now(),
            },
        );

        Ok(())
    }

    /// Unregister a consumer from this queue
    pub fn unregister_consumer(&self, consumer_id: u64) -> QueueResult<()> {
        let mut positions = self.consumer_positions.write().unwrap();
        positions.remove(&consumer_id);
        Ok(())
    }

    /// Get all registered consumer IDs
    pub fn consumer_ids(&self) -> Vec<u64> {
        self.consumer_positions
            .read()
            .unwrap()
            .keys()
            .copied()
            .collect()
    }

    /// Publish a message to the queue
    pub fn publish(&self, mut message: Message) -> QueueResult<u64> {
        // Check queue size limit
        {
            let messages = self.messages.read().unwrap();
            if messages.len() >= self.max_size {
                return Err(QueueError::QueueFull {
                    max_size: self.max_size,
                });
            }
        }

        // Get next sequence number atomically
        let sequence = {
            let mut next_seq = self.next_sequence.write().unwrap();
            let current = *next_seq;
            *next_seq += 1;
            current
        };

        // Update message header with assigned sequence
        message.header.sequence = sequence;

        // Add message to queue
        let entry = QueueEntry {
            sequence,
            message: Arc::new(message),
        };

        {
            let mut messages = self.messages.write().unwrap();
            messages.push_back(entry);
        }

        Ok(sequence)
    }

    /// Read next available message for a specific consumer
    pub fn read_next(&self, consumer_id: u64) -> QueueResult<Option<Arc<Message>>> {
        // Get current consumer position
        let current_position = {
            let positions = self.consumer_positions.read().unwrap();
            positions
                .get(&consumer_id)
                .ok_or_else(|| QueueError::ConsumerNotFound {
                    consumer_id: consumer_id.to_string(),
                })?
                .current_sequence
        };

        // Find next message at or after consumer's position
        let next_message = {
            let messages = self.messages.read().unwrap();
            messages
                .iter()
                .find(|entry| entry.sequence >= current_position)
                .cloned()
        };

        if let Some(entry) = next_message {
            // Update consumer position to next expected sequence
            {
                let mut positions = self.consumer_positions.write().unwrap();
                if let Some(pos) = positions.get_mut(&consumer_id) {
                    pos.current_sequence = entry.sequence + 1;
                    pos.last_read_timestamp = std::time::SystemTime::now();
                }
            }

            Ok(Some(entry.message))
        } else {
            Ok(None) // No more messages available
        }
    }

    /// Check if consumer exists
    pub fn has_consumer(&self, consumer_id: u64) -> bool {
        self.consumer_positions
            .read()
            .unwrap()
            .contains_key(&consumer_id)
    }

    /// Get consumer position information
    pub fn consumer_position(&self, consumer_id: u64) -> Option<u64> {
        self.consumer_positions
            .read()
            .unwrap()
            .get(&consumer_id)
            .map(|pos| pos.current_sequence)
    }

    /// Get consumer's last read timestamp
    pub fn consumer_last_read_time(&self, consumer_id: u64) -> Option<std::time::SystemTime> {
        self.consumer_positions
            .read()
            .unwrap()
            .get(&consumer_id)
            .map(|pos| pos.last_read_timestamp)
    }

    /// Calculate memory usage statistics for this queue
    pub fn memory_stats(&self) -> MemoryStats {
        let messages = self.messages.read().unwrap();
        let consumer_positions = self.consumer_positions.read().unwrap();

        let message_count = messages.len();

        // Calculate message data size
        let message_data_bytes: usize = messages
            .iter()
            .map(|entry| self.calculate_message_size(&entry.message))
            .sum();

        // Calculate Arc overhead (approximate)
        let arc_overhead = message_count * mem::size_of::<Arc<Message>>();

        // Calculate queue entry overhead
        let entry_overhead = message_count * mem::size_of::<QueueEntry>();

        // Calculate consumer position overhead
        let consumer_overhead = consumer_positions.len() * mem::size_of::<ConsumerPosition>();

        let overhead_bytes = arc_overhead + entry_overhead + consumer_overhead;

        MemoryStats {
            total_messages: message_count,
            total_bytes: message_data_bytes + overhead_bytes,
            message_data_bytes,
            overhead_bytes,
        }
    }

    /// Calculate the approximate memory size of a Message
    fn calculate_message_size(&self, message: &Message) -> usize {
        let header_size = mem::size_of::<MessageHeader>()
            + message.header.producer_id.len()
            + message.header.message_type.len();

        let data_size = message.data.len();

        header_size + data_size + mem::size_of::<u64>() // timestamp
    }

    /// Perform garbage collection based on consumer positions
    /// Removes messages that have been read by all consumers
    /// Returns the number of messages collected
    pub fn collect_garbage(&self) -> QueueResult<usize> {
        // Find the minimum sequence number across all consumers
        let min_sequence = match self.min_consumer_sequence() {
            Some(seq) => seq,
            None => return Ok(0), // No consumers, can't collect anything
        };

        // Remove messages with sequence < min_sequence
        let mut messages = self.messages.write().unwrap();
        let original_len = messages.len();

        // Keep messages that are >= min_sequence (not yet read by all consumers)
        messages.retain(|entry| entry.sequence >= min_sequence);

        let removed_count = original_len - messages.len();
        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_consumer_queue_creation() {
        let queue = MultiConsumerQueue::new("test-queue".to_string(), 1000);

        assert_eq!(queue.queue_id(), "test-queue");
        assert_eq!(queue.size(), 0);
        assert_eq!(queue.head_sequence(), 1);
        assert_eq!(queue.consumer_ids().len(), 0);
    }

    #[test]
    fn test_consumer_registration() {
        let queue = MultiConsumerQueue::new("test-queue".to_string(), 1000);

        // Register consumers
        assert!(queue.register_consumer(1).is_ok());
        assert!(queue.register_consumer(2).is_ok());

        // Check consumers are registered
        assert!(queue.has_consumer(1));
        assert!(queue.has_consumer(2));
        assert!(!queue.has_consumer(3));

        let consumer_ids = queue.consumer_ids();
        assert_eq!(consumer_ids.len(), 2);
        assert!(consumer_ids.contains(&1));
        assert!(consumer_ids.contains(&2));
    }

    #[test]
    fn test_consumer_unregistration() {
        let queue = MultiConsumerQueue::new("test-queue".to_string(), 1000);

        queue.register_consumer(1).unwrap();
        queue.register_consumer(2).unwrap();

        // Unregister one consumer
        assert!(queue.unregister_consumer(1).is_ok());

        assert!(!queue.has_consumer(1));
        assert!(queue.has_consumer(2));
        assert_eq!(queue.consumer_ids().len(), 1);
    }

    #[test]
    fn test_sequence_based_publish() {
        let queue = MultiConsumerQueue::new("test-queue".to_string(), 1000);

        // Create test messages
        let msg1 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        );
        let msg2 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file2.rs".to_string(),
        );

        // Publish messages should return monotonic sequence numbers
        let seq1 = queue.publish(msg1).unwrap();
        let seq2 = queue.publish(msg2).unwrap();

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(queue.size(), 2);
        assert_eq!(queue.head_sequence(), 3); // Next sequence to be assigned
    }

    #[test]
    fn test_queue_size_limit() {
        let queue = MultiConsumerQueue::new("test-producer".to_string(), 2); // Small limit

        let msg1 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        );
        let msg2 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file2.rs".to_string(),
        );
        let msg3 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file3.rs".to_string(),
        );

        // First two messages should succeed
        assert!(queue.publish(msg1).is_ok());
        assert!(queue.publish(msg2).is_ok());

        // Third message should fail with QueueFull error
        match queue.publish(msg3) {
            Err(QueueError::QueueFull { max_size }) => {
                assert_eq!(max_size, 2);
            }
            _ => panic!("Expected QueueFull error"),
        }
    }

    #[test]
    fn test_consumer_read_with_position_tracking() {
        let queue = MultiConsumerQueue::new("test-queue".to_string(), 1000);

        // Publish some messages first
        let msg1 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        );
        let msg2 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file2.rs".to_string(),
        );

        queue.publish(msg1).unwrap();
        queue.publish(msg2).unwrap();

        // Register consumer after messages are published
        queue.register_consumer(1).unwrap();

        // Consumer should start from current position (won't get historical messages)
        assert_eq!(queue.consumer_position(1), Some(3)); // Next sequence after msg2

        // Read should return None since consumer starts after published messages
        let result = queue.read_next(1).unwrap();
        assert!(result.is_none());

        // Publish a new message after consumer registration
        let msg3 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file3.rs".to_string(),
        );
        queue.publish(msg3).unwrap();

        // Now consumer should be able to read the new message
        let result = queue.read_next(1).unwrap();
        assert!(result.is_some());

        let message = result.unwrap();
        assert_eq!(message.data, "file3.rs");

        // Consumer position should be updated
        assert_eq!(queue.consumer_position(1), Some(4)); // Next after msg3
    }

    #[test]
    fn test_independent_consumer_positions() {
        let queue = MultiConsumerQueue::new("test-queue".to_string(), 1000);

        // Register consumers before publishing
        queue.register_consumer(1).unwrap();
        queue.register_consumer(2).unwrap();

        // Publish messages
        let msg1 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file1.rs".to_string(),
        );
        let msg2 = Message::new(
            "test-producer".to_string(),
            "file".to_string(),
            "file2.rs".to_string(),
        );

        queue.publish(msg1).unwrap();
        queue.publish(msg2).unwrap();

        // Both consumers should be able to read message 1
        let result1 = queue.read_next(1).unwrap().unwrap();
        let result2 = queue.read_next(2).unwrap().unwrap();

        assert_eq!(result1.data, "file1.rs");
        assert_eq!(result2.data, "file1.rs");

        // Only consumer 1 reads message 2
        let result1 = queue.read_next(1).unwrap().unwrap();
        assert_eq!(result1.data, "file2.rs");

        // Consumer 2 should still be able to read message 2
        let result2 = queue.read_next(2).unwrap().unwrap();
        assert_eq!(result2.data, "file2.rs");

        // Both consumers should now have no more messages
        assert!(queue.read_next(1).unwrap().is_none());
        assert!(queue.read_next(2).unwrap().is_none());
    }

    #[test]
    fn test_read_from_non_existent_consumer() {
        let queue = MultiConsumerQueue::new("test-queue".to_string(), 1000);

        // Try to read from unregistered consumer
        match queue.read_next(99) {
            Err(QueueError::ConsumerNotFound { consumer_id }) => {
                assert_eq!(consumer_id, "99");
            }
            _ => panic!("Expected ConsumerNotFound error"),
        }
    }
}
