# Multiconsumer Queue API Documentation

## Overview

The multiconsumer queue system provides a high-performance, generic producer/consumer messaging infrastructure for asynchronous communication between components. It implements a Kafka-like architecture with sequence-based ordering, independent consumer positions, and sophisticated memory management.

## Table of Contents

- [Architecture](#architecture)
- [Core Components](#core-components)
- [API Reference](#api-reference)
- [Usage Examples](#usage-examples)
- [Advanced Features](#advanced-features)
- [Performance Characteristics](#performance-characteristics)
- [Best Practices](#best-practices)

## Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Producer A  │     │  Producer B  │     │  Producer C  │
└──────┬───────┘     └──────┬───────┘     └──────┬───────┘
       │ publish            │ publish            │ publish
       ▼                    ▼                    ▼
┌───────────────────────────────────────────────────────┐
│              QueueManager (Global Singleton)          │
│  ┌────────────────────────────────────────────────┐   │
│  │         MultiConsumerQueue (Global Queue)      │   │
│  │  ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐     │   │
│  │  │ 1 │ 2 │ 3 │ 4 │ 5 │ 6 │ 7 │ 8 │ 9 │...│     │   │
│  │  └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘     │   │
│  │     ▲       ▲           ▲                      │   │
│  │     │       │           │                      │   │
│  └─────┼───────┼───────────┼──────────────────────┘   │
└────────┼───────┼───────────┼──────────────────────────┘
         │ read  │ read      │ read
┌────────┴──┐ ┌──┴───────┐ ┌─┴────────┐
│Consumer A │ │Consumer B│ │Consumer C│ (Independent positions)
└───────────┘ └──────────┘ └──────────┘
```

### Key Design Principles

- **Single Global Queue**: All messages flow through one queue, simplifying routing
- **Independent Consumers**: Each consumer maintains its own position in the message stream
- **Sequence-Based Ordering**: Monotonic sequence numbers guarantee message order
- **Arc-Wrapped Messages**: Zero-copy sharing between consumers for efficiency
- **Automatic Memory Management**: Backpressure and garbage collection prevent unbounded growth

## Core Components

### QueueManager

The central coordination point for all queue operations.

```rust
pub struct QueueManager {
    // Internal implementation
}
```

**Responsibilities:**
- Creates and manages publishers and consumers
- Maintains the global message queue
- Handles memory management and backpressure
- Monitors consumer lag and performs cleanup
- Integrates with the event notification system

### QueuePublisher

Lightweight handle for publishing messages to the queue.

```rust
pub struct QueuePublisher {
    producer_id: String,
    // Internal fields
}
```

**Features:**
- Thread-safe publishing
- Automatic sequence number assignment
- Memory pressure checking after each publish
- Weak reference to QueueManager (auto-cleanup on drop)

### QueueConsumer

Handle for reading messages from the queue with independent position tracking.

```rust
pub struct QueueConsumer {
    consumer_id: String,
    plugin_name: String,
    // Internal fields
}
```

**Features:**
- Independent read position
- Single and batch read operations
- Automatic registration/unregistration
- Thread-safe operations

### Message

Generic message structure for queue communication.

```rust
pub struct Message {
    pub header: MessageHeader,
    pub data: String,
}

pub struct MessageHeader {
    pub sequence: u64,
    pub timestamp: SystemTime,
    pub producer_id: String,
    pub message_type: String,
}
```

## API Reference

### QueueManager Methods

#### `create() -> Arc<QueueManager>`
Creates a new QueueManager instance and publishes a Started event.

```rust
let manager = QueueManager::create().await;
```

#### `create_publisher(producer_id: String) -> Result<QueuePublisher>`
Creates a new publisher with the specified producer ID.

```rust
let publisher = manager.create_publisher("my-service".to_string())?;
```

#### `create_consumer(plugin_name: String) -> Result<QueueConsumer>`
Creates a new consumer for the specified plugin.

```rust
let consumer = manager.create_consumer("my-plugin".to_string())?;
```

#### `set_memory_threshold_bytes(threshold: usize) -> Result<()>`
Sets the memory threshold for automatic garbage collection.

```rust
manager.set_memory_threshold_bytes(10_000_000)?; // 10MB
```

#### `memory_stats() -> MemoryStats`
Returns detailed memory usage statistics.

```rust
let stats = manager.memory_stats();
println!("Total messages: {}, Memory: {} bytes",
         stats.total_messages, stats.total_bytes);
```

#### `get_lag_statistics() -> Result<LagStats>`
Returns consumer lag statistics across all consumers.

```rust
let lag_stats = manager.get_lag_statistics()?;
println!("Max lag: {}, Avg lag: {:.1}",
         lag_stats.max_lag, lag_stats.avg_lag);
```

#### `detect_stale_consumers(threshold_seconds: u64) -> Result<Vec<StaleConsumerInfo>>`
Detects consumers that haven't read messages recently.

```rust
let stale = manager.detect_stale_consumers(60)?; // 60 second threshold
for consumer in stale {
    println!("Stale consumer: {}, lag: {}", consumer.consumer_id, consumer.lag);
}
```

#### `cleanup_stale_consumers(lag_threshold: usize) -> Result<usize>`
Removes stale consumers with lag above the threshold.

```rust
let removed = manager.cleanup_stale_consumers(100)?;
println!("Removed {} stale consumers", removed);
```

### QueuePublisher Methods

#### `publish(message: Message) -> Result<u64>`
Publishes a message to the queue and returns its sequence number.

```rust
let message = Message::new(
    "my-service".to_string(),
    "event_type".to_string(),
    "message data".to_string()
);
let sequence = publisher.publish(message)?;
```

### QueueConsumer Methods

#### `read() -> Result<Option<Arc<Message>>>`
Reads the next available message from the queue.

```rust
while let Some(message) = consumer.read()? {
    println!("Received: {}", message.data);
}
```

#### `read_batch(batch_size: usize) -> Result<Vec<Arc<Message>>>`
Reads up to `batch_size` messages from the queue.

```rust
let batch = consumer.read_batch(100)?;
for message in batch {
    process_message(message);
}
```

#### `acknowledge_batch(messages: &[Arc<Message>]) -> Result<usize>`
Acknowledges a batch of messages (currently a no-op, reserved for future at-least-once delivery).

```rust
let ack_count = consumer.acknowledge_batch(&batch)?;
```

### Message Methods

#### `new(producer_id: String, message_type: String, data: String) -> Message`
Creates a new message with the specified fields.

```rust
let message = Message::new(
    "file-scanner".to_string(),
    "file_discovered".to_string(),
    "/path/to/file.rs".to_string()
);
```

## Usage Examples

### Basic Producer/Consumer Pattern

```rust
use repostats::queue::{QueueManager, Message};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the queue system
    let manager = QueueManager::create().await;

    // Create a producer
    let publisher = manager.create_publisher("my-service".to_string())?;

    // Create a consumer
    let consumer = manager.create_consumer("my-processor".to_string())?;

    // Publish messages
    for i in 0..10 {
        let message = Message::new(
            "my-service".to_string(),
            "data".to_string(),
            format!("Message {}", i)
        );
        publisher.publish(message)?;
    }

    // Consume messages
    while let Some(message) = consumer.read()? {
        println!("Processing: {}", message.data);
    }

    Ok(())
}
```

### Multiple Consumers Pattern

```rust
use tokio::task;

// Create multiple consumers that process the same message stream
let consumers: Vec<_> = (0..3)
    .map(|i| manager.create_consumer(format!("worker-{}", i)).unwrap())
    .collect();

// Process messages concurrently
let mut handles = vec![];
for consumer in consumers {
    let handle = task::spawn(async move {
        while let Some(message) = consumer.read().unwrap() {
            // Each consumer processes all messages independently
            process_message(message).await;
        }
    });
    handles.push(handle);
}
```

### Batch Processing Pattern

```rust
// Process messages in batches for better performance
let consumer = manager.create_consumer("batch-processor".to_string())?;

loop {
    let batch = consumer.read_batch(100)?;

    if batch.is_empty() {
        // No messages available, wait before retry
        tokio::time::sleep(Duration::from_millis(100)).await;
        continue;
    }

    // Process batch efficiently
    process_batch(&batch).await?;

    // Acknowledge processing (for future at-least-once delivery)
    consumer.acknowledge_batch(&batch)?;
}
```

### Memory Management Pattern

```rust
// Configure memory management
manager.set_memory_threshold_bytes(50_000_000)?; // 50MB threshold

// Monitor memory usage
let stats = manager.memory_stats();
if stats.total_bytes > 40_000_000 {
    println!("Warning: High memory usage: {} MB",
             stats.total_bytes / 1_000_000);

    // Manually trigger garbage collection if needed
    manager.collect_garbage()?;
}

// Monitor consumer lag
let lag_stats = manager.get_lag_statistics()?;
if lag_stats.max_lag > 1000 {
    println!("Warning: Consumer lag detected: {} messages",
             lag_stats.max_lag);

    // Clean up stale consumers
    manager.cleanup_stale_consumers(500)?;
}
```

## Advanced Features

### Message Grouping

Messages can implement the `GroupedMessage` trait for batch coordination:

```rust
use repostats::queue::{Message, GroupedMessage};

struct BatchMessage {
    inner: Message,
    batch_id: String,
    batch_size: Option<usize>,
}

impl GroupedMessage for BatchMessage {
    fn group_id(&self) -> Option<String> {
        Some(self.batch_id.clone())
    }

    fn starts_group(&self) -> Option<(String, usize)> {
        self.batch_size.map(|size| (self.batch_id.clone(), size))
    }

    fn completes_group(&self) -> Option<String> {
        None // Use count-based completion
    }
}
```

### Consumer Lag Monitoring

Monitor and manage consumer lag to ensure healthy message processing:

```rust
// Get lag for a specific consumer
let lag = manager.get_consumer_lag(&consumer)?;
if lag > 100 {
    println!("Consumer is {} messages behind", lag);
}

// Get overall lag statistics
let stats = manager.get_lag_statistics()?;
println!("Consumers: {}, Max lag: {}, Avg lag: {:.1}",
         stats.total_consumers, stats.max_lag, stats.avg_lag);

// Detect stale consumers
let stale = manager.detect_stale_consumers(30)?; // 30 second threshold
for info in stale {
    println!("Stale consumer {} with lag {}",
             info.consumer_id, info.lag);
}
```

### Backpressure Handling

The system automatically handles memory pressure:

```rust
// Set memory threshold
manager.set_memory_threshold_bytes(100_000_000)?; // 100MB

// Publishing automatically triggers cleanup on memory pressure
for i in 0..10000 {
    let message = create_large_message(i);

    match publisher.publish(message) {
        Ok(seq) => println!("Published: {}", seq),
        Err(e) => {
            // Handle backpressure or other errors
            println!("Failed to publish: {:?}", e);
            break;
        }
    }
}

// Check if memory pressure was handled
if manager.check_memory_pressure()? {
    println!("Memory pressure detected and handled");
}
```

## Performance Characteristics

### Throughput

- **Publishing**: O(1) for single message publish
- **Reading**: O(1) for single message read
- **Batch Reading**: O(n) where n is batch size
- **Memory**: Arc-wrapped messages enable zero-copy sharing

### Capacity

- **Message Flooding**: Successfully tested with 10,000+ messages
- **Large Messages**: Handles 1MB+ messages efficiently
- **Concurrent Consumers**: Tested with 5+ concurrent consumers
- **Memory Management**: Automatic garbage collection prevents unbounded growth

### Benchmarks

Based on integration tests:

- **Concurrent Processing**: 5 consumers processing 500 messages in ~130ms
- **Message Flood**: 10,000 messages processed with ~1.3MB memory usage
- **Large Messages**: 1MB message handled with minimal overhead
- **Backpressure**: 150 messages published under memory pressure

## Best Practices

### 1. Consumer Lifecycle Management

Always ensure consumers are properly cleaned up:

```rust
{
    let consumer = manager.create_consumer("temp-consumer".to_string())?;
    // Use consumer...
} // Consumer automatically unregisters on drop
```

### 2. Batch Processing for Performance

Use batch reading for better throughput:

```rust
// Good: Process in batches
let batch = consumer.read_batch(100)?;
process_batch(&batch);

// Less efficient: Process one at a time
while let Some(msg) = consumer.read()? {
    process_single(msg);
}
```

### 3. Memory Monitoring

Regularly monitor memory usage in production:

```rust
tokio::spawn(async move {
    loop {
        let stats = manager.memory_stats();
        metrics::gauge!("queue.memory.bytes", stats.total_bytes as f64);
        metrics::gauge!("queue.messages.count", stats.total_messages as f64);

        tokio::time::sleep(Duration::from_secs(10)).await;
    }
});
```

### 4. Error Handling

Always handle queue errors gracefully:

```rust
match consumer.read() {
    Ok(Some(message)) => process_message(message),
    Ok(None) => {
        // No messages available, wait before retry
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(e) => {
        // Log error and potentially recreate consumer
        error!("Failed to read message: {:?}", e);
        consumer = manager.create_consumer("my-consumer".to_string())?;
    }
}
```

### 5. Producer ID Conventions

Use meaningful producer IDs for debugging:

```rust
// Good: Descriptive producer IDs
let publisher = manager.create_publisher("file-scanner-v2".to_string())?;

// Less helpful: Generic IDs
let publisher = manager.create_publisher("producer1".to_string())?;
```

## Thread Safety

All queue components are designed to be thread-safe:

- **QueueManager**: Can be shared via `Arc<QueueManager>`
- **QueuePublisher**: Thread-safe publishing operations
- **QueueConsumer**: Thread-safe reading with independent positions
- **Message**: Immutable and Arc-wrapped for safe sharing

## Error Handling

The queue system uses a unified `QueueError` type:

```rust
pub enum QueueError {
    QueueFull { queue_name: String },
    ConsumerNotFound { consumer_id: u64 },
    ProducerNotFound { producer_id: String },
    SequenceOutOfBounds { sequence: u64 },
    OperationFailed { message: String },
    InvalidConfiguration { message: String },
}
```

## Integration with Event System

The queue system publishes lifecycle events:

```rust
// Queue lifecycle events
pub enum QueueEventType {
    Started,        // Queue system initialized
    Shutdown,       // Queue system shutting down
    MessageAdded,   // Message published (with optional grouping)
    MemoryLow,      // Memory pressure detected
    MemoryNormal,   // Memory pressure resolved
}

// Subscribe to queue events
let mut subscriber = notification_manager.subscribe(
    "queue-monitor".to_string(),
    vec![EventType::Queue]
).await?;

while let Ok(event) = subscriber.receive().await {
    if let Event::Queue(queue_event) = event {
        handle_queue_event(queue_event);
    }
}
```

## Limitations and Future Enhancements

### Current Limitations

1. **Persistence**: Messages are in-memory only (no disk persistence)
2. **Delivery Guarantees**: At-most-once delivery (no acknowledgment-based retry)
3. **Message Format**: String-only payloads (no binary support)
4. **Partitioning**: Single global queue (no topic partitioning)

### Planned Enhancements

1. **At-Least-Once Delivery**: Acknowledgment-based message replay
2. **Binary Payloads**: Support for arbitrary byte arrays
3. **Topic Partitioning**: Multiple queues with routing rules
4. **Persistence Options**: Optional disk-based message storage
5. **Dead Letter Queue**: Automatic handling of failed messages

## Migration Guide

### From Direct Function Calls

Before:
```rust
// Direct synchronous call
let result = process_file(file_path);
```

After:
```rust
// Asynchronous via queue
let message = Message::new(
    "file-processor".to_string(),
    "process_file".to_string(),
    file_path
);
publisher.publish(message)?;

// In consumer
while let Some(msg) = consumer.read()? {
    let result = process_file(&msg.data);
}
```

### From Channel-Based Communication

Before:
```rust
let (tx, rx) = mpsc::channel();
tx.send(data)?;
let received = rx.recv()?;
```

After:
```rust
// Publisher side
let message = Message::new("sender".to_string(), "data".to_string(), data);
publisher.publish(message)?;

// Consumer side
let message = consumer.read()?.unwrap();
let received = message.data;
```

## Troubleshooting

### High Memory Usage

```rust
// Check memory statistics
let stats = manager.memory_stats();
println!("Total messages: {}", stats.total_messages);
println!("Memory usage: {} MB", stats.total_bytes / 1_000_000);

// Force garbage collection
let collected = manager.collect_garbage()?;
println!("Collected {} messages", collected);

// Check for stale consumers
let stale = manager.detect_stale_consumers(0)?;
println!("Found {} stale consumers", stale.len());
```

### Consumer Lag

```rust
// Identify slow consumers
let lag_stats = manager.get_lag_statistics()?;
for consumer in manager.detect_stale_consumers(0)? {
    if consumer.lag > 100 {
        println!("Consumer {} is {} messages behind",
                 consumer.consumer_id, consumer.lag);
    }
}

// Clean up stale consumers
manager.cleanup_stale_consumers(100)?;
```

### Message Loss

Messages are only "lost" if no consumer reads them before garbage collection:

```rust
// Ensure consumers are created before publishing
let consumer = manager.create_consumer("critical-consumer".to_string())?;
// Now safe to publish
publisher.publish(message)?;
```

## Support and Contributing

For issues, feature requests, or contributions, please refer to the project's GitHub repository.

## License

This component is part of the repostats project and follows the same licensing terms.
