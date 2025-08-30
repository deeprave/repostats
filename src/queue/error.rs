//! Queue Error Types

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue is full (max size: {max_size})")]
    QueueFull { max_size: usize },

    #[error("Consumer not found: {consumer_id}")]
    ConsumerNotFound { consumer_id: String },

    #[error("Producer not found: {producer_id}")]
    ProducerNotFound { producer_id: String },

    #[error("Sequence out of bounds: {sequence}")]
    SequenceOutOfBounds { sequence: u64 },

    #[error("Operation failed: {message}")]
    OperationFailed { message: String },
}

/// Result type for queue operations
pub type QueueResult<T> = Result<T, QueueError>;
