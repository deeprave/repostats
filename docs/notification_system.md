# Notification System Documentation

## Overview

The notification system provides an event-driven publisher-subscriber architecture for communication between different components of the repostats application. It enables loose coupling between system components while maintaining type safety and high performance.

## Architecture

The notification system is built around these core components:

- **Events**: Type-safe event variants for different system components (Scan, Queue, Plugin, System)
- **Event Filters**: Configurable filtering to receive only relevant events
- **AsyncNotificationManager**: Core manager handling subscriptions and event publishing
- **ServiceRegistry**: Global singleton providing thread-safe access to the notification manager
- **Subscriber Statistics**: Comprehensive monitoring and health tracking
- **Error Handling**: Robust error types with graceful degradation

## Quick Start

```rust
use repostats::core::services;
use repostats::notifications::api::{EventFilter, Event, ScanEvent, ScanEventType};

// Access the global notification manager
let mut manager = services::get_services().notification_manager();

// Subscribe to scan events only
let mut receiver = manager.subscribe(
    "my_scanner".to_string(),
    EventFilter::ScanOnly,
    "plugin:file_scanner".to_string()
)?;

// Publish a scan event
let event = Event::Scan(ScanEvent::new(
    ScanEventType::Started,
    "scan_123".to_string()
));
manager.publish(event)?;

// Receive the event
if let Some(received_event) = receiver.recv().await {
    println!("Received: {:?}", received_event);
}
```

## Event Types

### Scan Events
Events related to file system scanning operations:
- `Started`: Scan operation begins
- `Progress`: Periodic progress updates
- `DataReady`: Scan data is available for processing
- `Warning`: Non-critical issues during scanning
- `Error`: Critical errors that may stop scanning
- `Completed`: Scan operation finished successfully

### Queue Events
Events for message queue operations:
- `MessageAdded`: New message added to queue
- `MessageProcessed`: Message successfully processed
- `QueueEmpty`: Queue has no pending messages

### Plugin Events
Events for plugin lifecycle management:
- `Registered`: Plugin registered with the system
- `Processing`: Plugin is actively processing data
- `DataReady`: Plugin has data ready for consumption
- `Error`: Plugin encountered an error
- `Unregistered`: Plugin removed from system

### System Events
System-wide operational events:
- `Startup`: System initialization complete
- `Shutdown`: System shutdown initiated

## Event Filtering

Subscribers can filter events using `EventFilter`:
- `ScanOnly`: Only scan-related events
- `QueueOnly`: Only queue-related events
- `PluginOnly`: Only plugin-related events
- `SystemOnly`: Only system-wide events
- `ScanAndPlugin`: Scan and plugin events
- `QueueAndSystem`: Queue and system events
- `All`: All event types

## Service Registry

The ServiceRegistry provides centralized access to core services using the singleton pattern with lazy initialization:

```rust
use repostats::core::services;

// Get the global service registry
let services = services::get_services();

// Access the notification manager (returns MutexGuard)
let mut manager = services.notification_manager();

// Use the manager
let count = manager.subscriber_count();
```

## Auto-Management Features

The notification manager includes built-in auto-management capabilities:

### Queue Size Monitoring
- Tracks queue sizes per subscriber
- High water mark detection at 10,000 queued events
- Automatic warning generation for overloaded subscribers

### Stale Subscriber Detection
- Identifies subscribers that stop consuming events
- Automatic removal after 5 minutes of inactivity with high queue size
- Prevents memory leaks from abandoned subscribers

### Error Rate Limiting
- Prevents log flooding from problematic subscribers
- Minimum 60-second interval between error logs
- Tracks error rate and throttles logging when threshold (10%) exceeded

### Memory Protection
- Detects memory exhaustion (>1M total queued events)
- Prevents system-wide failures from runaway event generation
- Graceful degradation when individual subscribers fail

### System Overload Detection
- Monitors total active subscribers (warning at 1000+)
- Tracks ratio of problematic subscribers
- Generates alerts when system health degrades

## Threading and Safety

- All operations are thread-safe using atomic counters and mutexes
- The global ServiceRegistry uses LazyLock for safe initialization
- Events are delivered via unbounded channels for high throughput
- Interior mutability allows shared access to the notification manager
- Multiple threads can safely access the service registry concurrently

## Error Handling

The system uses `NotificationError` for comprehensive error reporting:

- `SubscriberNotFound`: Attempt to access non-existent subscriber
- `ChannelClosed`: Subscriber channel was closed
- `PublishFailed`: Event publishing failed for some subscribers (includes list of failed IDs)
- `Fatal`: Critical system errors requiring intervention
- `OutOfMemory`: Memory exhaustion detected (includes queue sizes and total events)
- `SystemOverload`: Too many problematic subscribers (includes counts and statistics)

## Performance Characteristics

- **Unbounded Channels**: High-throughput event delivery without blocking publishers
- **Lazy Initialization**: Services initialized only when first accessed
- **Atomic Operations**: Lock-free statistics tracking for minimal overhead
- **Batch Operations**: Efficient handling of multiple subscribers
- **Automatic Cleanup**: Dropped subscribers removed automatically

## Best Practices

1. **Subscriber Naming**: Use descriptive IDs like `"scanner:file_processor"` or `"plugin:export"`
2. **Source Tracking**: Always provide meaningful source strings for debugging
3. **Event Filtering**: Use specific filters to reduce unnecessary event processing
4. **Resource Cleanup**: Subscribers automatically cleaned up when receivers drop
5. **Error Handling**: Check for `PublishFailed` errors to detect problematic subscribers
6. **Health Monitoring**: Periodically check subscriber statistics for system health

## Example: Plugin Integration

```rust
use repostats::core::services;
use repostats::notifications::api::{
    EventFilter, Event, PluginEvent, PluginEventType
};

pub struct MyPlugin {
    id: String,
}

impl MyPlugin {
    pub async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = services::get_services().notification_manager();

        // Subscribe to relevant events
        let mut receiver = manager.subscribe(
            self.id.clone(),
            EventFilter::ScanAndPlugin,
            format!("plugin:{}", self.id)
        )?;

        // Announce plugin registration
        let event = Event::Plugin(PluginEvent::new(
            PluginEventType::Registered,
            self.id.clone()
        ));
        manager.publish(event)?;

        // Process events in background
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                // Handle events
                match event {
                    Event::Scan(scan) => self.handle_scan_event(scan),
                    Event::Plugin(plugin) => self.handle_plugin_event(plugin),
                    _ => {}
                }
            }
        });

        Ok(())
    }
}
```

## Debugging and Monitoring

### Subscriber Statistics

Access real-time statistics for any subscriber:

```rust
let stats = manager.get_subscriber_statistics("my_subscriber");
if let Some(stats) = stats {
    println!("Queue size: {}", stats.queue_size.load(Ordering::Relaxed));
    println!("Messages processed: {}", stats.messages_processed.load(Ordering::Relaxed));
    println!("Error count: {}", stats.error_count.load(Ordering::Relaxed));
}
```

### Health Assessment

Get overall system health:

```rust
let health = manager.assess_subscriber_health();
println!("Healthy subscribers: {}", health.healthy_count);
println!("Warning subscribers: {}", health.warning_count);
println!("Critical subscribers: {}", health.critical_count);
```

### Log Output

The system generates contextual log messages with operational intelligence:

```
WARN Event processing failed for subscriber 'scan_logger' (source: logger:scan_events) - 5 errors in 100 messages over 60s since last log
```

## Migration Guide

For plugins migrating to the notification system:

1. Replace direct function calls with event publishing
2. Subscribe to relevant event types using filters
3. Implement async event handlers
4. Add source tracking for debugging
5. Monitor subscriber statistics for health

## Configuration Constants

The system uses these hardcoded constants (future versions may make these configurable):

- `HIGH_WATER_MARK`: 10,000 - Queue size threshold for concern
- `STALE_SUBSCRIBER_TIMEOUT`: 5 minutes - Time without consuming before removal
- `MIN_ERROR_LOG_INTERVAL`: 60 seconds - Minimum time between error logs
- `ERROR_RATE_THRESHOLD`: 10% - Error rate that triggers log throttling
- `MEMORY_EXHAUSTION_THRESHOLD`: 1,000,000 - Total events before memory warning
- `SYSTEM_OVERLOAD_THRESHOLD`: 1,000 - Maximum recommended subscribers