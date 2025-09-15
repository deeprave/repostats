//! Tests for TypedQueueConsumer functionality
//!
//! This test suite verifies that the typed queue consumer system works correctly,
//! including successful deserialization, error handling, and metadata access.

use crate::queue::api::{Message, QueueManager};
use crate::queue::error::QueueError;
use crate::queue::typed::TypedQueueManagerExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestMessage {
    content: String,
    value: i32,
}

#[tokio::test]
async fn test_typed_consumer_successful_deserialization() {
    let manager = Arc::new(QueueManager::new());

    // Create publisher and typed consumer
    let publisher = manager
        .create_publisher("test-producer".to_string())
        .unwrap();
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("test-consumer".to_string())
        .unwrap();

    // Test successful deserialization
    let test_message = TestMessage {
        content: "Hello World".to_string(),
        value: 42,
    };
    let json_data = serde_json::to_string(&test_message).unwrap();
    let message = Message::new("test-producer".to_string(), "test".to_string(), json_data);

    publisher.publish(message).unwrap();

    let received = typed_consumer.read().unwrap();
    assert!(received.is_some());
    let received_msg = received.unwrap();
    assert_eq!(received_msg.content, "Hello World");
    assert_eq!(received_msg.value, 42);
}

#[tokio::test]
async fn test_typed_consumer_deserialization_error() {
    let manager = Arc::new(QueueManager::new());

    let publisher = manager
        .create_publisher("test-producer".to_string())
        .unwrap();
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("test-consumer".to_string())
        .unwrap();

    // Test invalid JSON
    let message = Message::new(
        "test-producer".to_string(),
        "test".to_string(),
        "invalid json".to_string(),
    );
    publisher.publish(message).unwrap();

    let result = typed_consumer.read();
    assert!(result.is_err());
    match result.unwrap_err() {
        QueueError::DeserializationError { message } => {
            assert!(message.contains("Failed to deserialize message to"));
            assert!(message.contains("TestMessage"));
        }
        other => panic!("Expected DeserializationError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_typed_consumer_with_header_metadata() {
    let manager = Arc::new(QueueManager::new());

    let publisher = manager
        .create_publisher("test-producer".to_string())
        .unwrap();
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("test-consumer".to_string())
        .unwrap();

    let test_message = TestMessage {
        content: "Test".to_string(),
        value: 123,
    };
    let json_data = serde_json::to_string(&test_message).unwrap();
    let message = Message::new(
        "test-producer".to_string(),
        "test_type".to_string(),
        json_data,
    );
    publisher.publish(message).unwrap();

    let received = typed_consumer.read_with_header().unwrap().unwrap();

    // Verify typed content
    assert_eq!(received.content.content, "Test");
    assert_eq!(received.content.value, 123);

    // Verify header metadata
    assert_eq!(received.producer_id(), "test-producer");
    assert_eq!(received.message_type(), "test_type");
    assert!(received.sequence() > 0);
}

#[tokio::test]
async fn test_typed_consumer_empty_queue() {
    let manager = Arc::new(QueueManager::new());
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("test-consumer".to_string())
        .unwrap();

    // Should return None for empty queue
    let result = typed_consumer.read().unwrap();
    assert!(result.is_none());

    let result_with_header = typed_consumer.read_with_header().unwrap();
    assert!(result_with_header.is_none());
}

#[tokio::test]
async fn test_typed_consumer_multiple_messages() {
    let manager = Arc::new(QueueManager::new());

    let publisher = manager
        .create_publisher("test-producer".to_string())
        .unwrap();
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("test-consumer".to_string())
        .unwrap();

    // Publish multiple messages
    for i in 0..3 {
        let test_message = TestMessage {
            content: format!("Message {}", i),
            value: i,
        };
        let json_data = serde_json::to_string(&test_message).unwrap();
        let message = Message::new("test-producer".to_string(), "test".to_string(), json_data);
        publisher.publish(message).unwrap();
    }

    // Read all messages in order
    for expected_i in 0..3 {
        let received = typed_consumer.read().unwrap();
        assert!(received.is_some());
        let received_msg = received.unwrap();
        assert_eq!(received_msg.content, format!("Message {}", expected_i));
        assert_eq!(received_msg.value, expected_i);
    }

    // Queue should be empty now
    let result = typed_consumer.read().unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_enhanced_deserialization_error_context() {
    let manager = Arc::new(QueueManager::new());

    let publisher = manager
        .create_publisher("context-producer".to_string())
        .unwrap();
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("context-consumer".to_string())
        .unwrap();

    // Test with invalid JSON that's longer than 100 characters to test data preview truncation
    let long_invalid_json = format!("{{\"invalid\": \"{}\"}}", "x".repeat(150));
    let message = Message::new(
        "context-producer".to_string(),
        "context_type".to_string(),
        long_invalid_json.clone(),
    );
    publisher.publish(message).unwrap();

    let result = typed_consumer.read();
    assert!(result.is_err());
    match result.unwrap_err() {
        QueueError::DeserializationError { message } => {
            // Verify enhanced error context includes all expected metadata
            assert!(message.contains("TestMessage"));
            assert!(message.contains("sequence:"));
            assert!(message.contains("type: 'context_type'"));
            assert!(message.contains("producer: 'context-producer'"));
            assert!(message.contains("data_length:"));
            assert!(message.contains("data_preview:"));

            // Verify data length is correct
            assert!(message.contains(&format!("data_length: {}", long_invalid_json.len())));

            // Verify data preview is truncated with ellipsis for long messages
            assert!(message.contains("..."));
        }
        other => panic!(
            "Expected DeserializationError with enhanced context, got: {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_short_message_error_context() {
    let manager = Arc::new(QueueManager::new());

    let publisher = manager
        .create_publisher("short-producer".to_string())
        .unwrap();
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("short-consumer".to_string())
        .unwrap();

    // Test with short invalid JSON (no truncation expected)
    let short_invalid_json = "invalid";
    let message = Message::new(
        "short-producer".to_string(),
        "short_type".to_string(),
        short_invalid_json.to_string(),
    );
    publisher.publish(message).unwrap();

    let result = typed_consumer.read();
    assert!(result.is_err());
    match result.unwrap_err() {
        QueueError::DeserializationError { message } => {
            // Verify enhanced error context
            assert!(message.contains("TestMessage"));
            assert!(message.contains("sequence:"));
            assert!(message.contains("type: 'short_type'"));
            assert!(message.contains("producer: 'short-producer'"));
            assert!(message.contains("data_length: 7")); // "invalid" is 7 chars
            assert!(message.contains("data_preview: 'invalid'")); // No truncation

            // Should NOT contain ellipsis for short messages
            assert!(!message.contains("..."));
        }
        other => panic!(
            "Expected DeserializationError with enhanced context, got: {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_typed_consumer_inner_access() {
    let manager = Arc::new(QueueManager::new());
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("test-consumer".to_string())
        .unwrap();

    // Should provide access to underlying consumer
    let inner = typed_consumer.inner();
    assert_eq!(inner.plugin_name(), "test-consumer");
}

#[tokio::test]
async fn test_typed_message_metadata_methods() {
    let manager = Arc::new(QueueManager::new());

    let publisher = manager
        .create_publisher("metadata-producer".to_string())
        .unwrap();
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("metadata-consumer".to_string())
        .unwrap();

    let test_message = TestMessage {
        content: "Metadata Test".to_string(),
        value: 999,
    };
    let json_data = serde_json::to_string(&test_message).unwrap();
    let message = Message::new(
        "metadata-producer".to_string(),
        "metadata_type".to_string(),
        json_data,
    );

    publisher.publish(message).unwrap();
    let typed_msg = typed_consumer.read_with_header().unwrap().unwrap();

    // Test all metadata methods
    assert!(typed_msg.sequence() > 0);
    assert_eq!(typed_msg.producer_id(), "metadata-producer");
    assert_eq!(typed_msg.message_type(), "metadata_type");
}

#[tokio::test]
async fn test_typed_consumer_mixed_message_types() {
    let manager = Arc::new(QueueManager::new());

    let publisher = manager
        .create_publisher("mixed-producer".to_string())
        .unwrap();
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("mixed-consumer".to_string())
        .unwrap();

    // Publish a valid TestMessage
    let valid_message = TestMessage {
        content: "Valid".to_string(),
        value: 100,
    };
    let valid_json = serde_json::to_string(&valid_message).unwrap();
    let valid_msg = Message::new("mixed-producer".to_string(), "test".to_string(), valid_json);
    publisher.publish(valid_msg).unwrap();

    // Publish an invalid message (different structure)
    let invalid_json = r#"{"different": "structure", "number": 42}"#;
    let invalid_msg = Message::new(
        "mixed-producer".to_string(),
        "test".to_string(),
        invalid_json.to_string(),
    );
    publisher.publish(invalid_msg).unwrap();

    // First message should deserialize successfully
    let first = typed_consumer.read().unwrap().unwrap();
    assert_eq!(first.content, "Valid");
    assert_eq!(first.value, 100);

    // Second message should fail deserialization
    let second = typed_consumer.read();
    assert!(second.is_err());
    match second.unwrap_err() {
        QueueError::DeserializationError { message } => {
            assert!(message.contains("TestMessage"));
        }
        other => panic!("Expected DeserializationError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_binary_data_error_context() {
    let manager = Arc::new(QueueManager::new());

    let publisher = manager
        .create_publisher("binary-producer".to_string())
        .unwrap();
    let typed_consumer = manager
        .create_typed_consumer::<TestMessage>("binary-consumer".to_string())
        .unwrap();

    // Create binary data with invalid UTF-8 sequences
    let mut binary_data = b"valid start ".to_vec();
    binary_data.extend_from_slice(&[0xFF, 0xFE, 0xFD, 0xFC]); // Invalid UTF-8
    binary_data
        .extend_from_slice(b" more content that exceeds 100 bytes to test truncation behaviour ");
    binary_data.extend_from_slice(&[0x80, 0x81, 0x82, 0x83]); // More invalid UTF-8
    binary_data.extend_from_slice(&[65u8; 50]); // 50 'A's to make it long

    let binary_string = String::from_utf8_lossy(&binary_data).to_string();
    let message = Message::new(
        "binary-producer".to_string(),
        "binary_type".to_string(),
        binary_string,
    );
    publisher.publish(message).unwrap();

    let result = typed_consumer.read();
    assert!(result.is_err());
    match result.unwrap_err() {
        QueueError::DeserializationError { message } => {
            // Verify enhanced error context handles binary data gracefully
            assert!(message.contains("TestMessage"));
            assert!(message.contains("sequence:"));
            assert!(message.contains("type: 'binary_type'"));
            assert!(message.contains("producer: 'binary-producer'"));
            assert!(message.contains("data_length:"));
            assert!(message.contains("data_preview:"));

            // Should contain ellipsis for truncated data
            assert!(message.contains("..."));

            // The error message should be properly formed despite invalid UTF-8
            assert!(!message.is_empty());
        }
        other => panic!(
            "Expected DeserializationError with binary data handling, got: {:?}",
            other
        ),
    }
}
