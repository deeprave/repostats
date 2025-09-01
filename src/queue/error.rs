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

impl crate::core::error_handling::ContextualError for QueueError {
    fn is_user_actionable(&self) -> bool {
        match self {
            QueueError::QueueFull { .. } => true, // User can adjust concurrency/batch size
            QueueError::ConsumerNotFound { .. } => true, // User can check plugin config
            QueueError::ProducerNotFound { .. } => true, // User can check plugin config
            QueueError::SequenceOutOfBounds { .. } => false, // System/timing issue
            QueueError::OperationFailed { .. } => false, // System error
        }
    }

    fn user_message(&self) -> Option<&str> {
        match self {
            QueueError::QueueFull { .. } => {
                Some("Queue is full. Try reducing batch size or concurrent operations.")
            }
            QueueError::ConsumerNotFound { .. } => {
                Some("Consumer configuration error. Check your plugin configuration.")
            }
            QueueError::ProducerNotFound { .. } => {
                Some("Producer configuration error. Check your plugin configuration.")
            }
            _ => None,
        }
    }
}

/// Result type for queue operations
pub type QueueResult<T> = Result<T, QueueError>;
