# Service Registry Documentation

## Overview

The ServiceRegistry provides centralized, thread-safe access to core services throughout the repostats application. It implements the singleton pattern with lazy initialization to ensure efficient resource usage while maintaining consistent availability.

## Architecture

The ServiceRegistry follows these design principles:

- **Singleton Pattern**: Single global instance accessible throughout the application
- **Lazy Initialization**: Services initialized only when first accessed using `LazyLock`
- **Thread Safety**: All operations safe for concurrent access from multiple threads
- **Interior Mutability**: Services requiring mutable access wrapped in appropriate synchronization primitives

## Core Components

### LazyLock Singleton

The global service registry is initialized using Rust's `LazyLock`:

```rust
pub static SERVICES: LazyLock<ServiceRegistry> = LazyLock::new(|| ServiceRegistry::new());
```

This ensures:
- Thread-safe initialization on first access
- No runtime overhead after initialization
- Guaranteed single instance across the application

### Service Access

Services are accessed through the registry with appropriate synchronization:

```rust
pub fn get_services() -> &'static ServiceRegistry {
    &SERVICES
}
```

## Usage Examples

### Basic Access

```rust
use repostats::core::services;

// Get the global service registry
let services = services::get_services();

// Services are now available for use
```

### Notification Manager Access

```rust
use repostats::core::services;
use repostats::notifications::api::{EventFilter, Event, SystemEvent, SystemEventType};

// Get the global service registry
let services = services::get_services();

// Access the notification manager (returns MutexGuard)
let mut manager = services.notification_manager();

// Use the manager while the guard is in scope
let receiver = manager.subscribe(
    "my_service".to_string(),
    EventFilter::All,
    "service:example".to_string()
).expect("Failed to subscribe");

// Publish system events
let event = Event::System(SystemEvent::new(SystemEventType::Startup));
manager.publish(event).expect("Failed to publish event");

// Guard automatically releases when it goes out of scope
```

### Multi-threaded Access

```rust
use std::thread;
use repostats::core::services;

// Multiple threads can safely access the service registry
let handles: Vec<_> = (0..10).map(|i| {
    thread::spawn(move || {
        let services = services::get_services();
        let manager = services.notification_manager();

        // Each thread gets its own MutexGuard
        // Work with the notification manager
        let count = manager.subscriber_count();
        println!("Thread {}: {} subscribers", i, count);
    })
}).collect();

// Wait for all threads
for handle in handles {
    handle.join().unwrap();
}
```

## Service Lifecycle

### Initialization

1. First call to `get_services()` triggers `LazyLock::new()`
2. `ServiceRegistry::new()` is called exactly once
3. All services are initialized with default configurations
4. Registry becomes available for the application lifetime

### Service Wrapping

Services requiring mutable access are wrapped in synchronization primitives:

```rust
pub struct ServiceRegistry {
    notification_manager: Mutex<AsyncNotificationManager>,
    // Future services would be added here
}
```

This allows:
- Safe concurrent access from multiple threads
- Mutable operations through `MutexGuard`
- Automatic lock release when guards go out of scope

### Cleanup

Services remain available for the entire application lifetime. Cleanup occurs automatically when the process terminates.

## Thread Safety Guarantees

### Initialization Safety
- `LazyLock` ensures exactly one initialization
- No race conditions during first access
- Thread-safe static initialization

### Access Safety
- Multiple threads can safely call `get_services()`
- Each service access returns appropriate guards
- Automatic lock management prevents deadlocks

### Service Safety
- Services wrapped in appropriate synchronization primitives
- Mutex prevents data races on mutable operations
- Guards provide compile-time safety guarantees

## Performance Characteristics

### Initialization
- **One-time Cost**: Initialization occurs only on first access
- **Zero Overhead**: No runtime cost after initialization
- **Lazy Loading**: Services not accessed have minimal impact

### Access Patterns
- **Registry Access**: Zero-cost static reference
- **Service Access**: Single mutex lock per operation
- **Lock Contention**: Minimal due to short critical sections

### Memory Usage
- **Single Instance**: Only one registry exists globally
- **Service Storage**: Services stored directly in registry
- **No Allocation**: No dynamic allocation after initialization

## Error Handling

### Mutex Poisoning

The registry uses `.unwrap()` on mutex operations, which will panic if a mutex is poisoned:

```rust
pub fn notification_manager(&self) -> std::sync::MutexGuard<'_, AsyncNotificationManager> {
    self.notification_manager.lock().unwrap()
}
```

This design choice assumes:
- Service methods should not panic during normal operation
- A poisoned mutex indicates a serious system error
- Failing fast is better than attempting recovery

### Service Failures

Individual service failures do not affect the registry itself:
- Registry remains accessible even if services encounter errors
- Services can implement their own error recovery
- Failed operations return appropriate error types

## Extension Patterns

### Adding New Services

To add a new service to the registry:

1. Add the service field to `ServiceRegistry`
2. Initialize it in `ServiceRegistry::new()`
3. Add an accessor method with appropriate synchronization

```rust
pub struct ServiceRegistry {
    notification_manager: Mutex<AsyncNotificationManager>,
    // New service example
    file_manager: Arc<FileManager>,  // Example: shared immutable service
    cache_manager: Mutex<CacheManager>,  // Example: mutable service
}

impl ServiceRegistry {
    fn new() -> Self {
        Self {
            notification_manager: Mutex::new(AsyncNotificationManager::new()),
            file_manager: Arc::new(FileManager::new()),
            cache_manager: Mutex::new(CacheManager::new()),
        }
    }

    pub fn file_manager(&self) -> &FileManager {
        &self.file_manager
    }

    pub fn cache_manager(&self) -> std::sync::MutexGuard<'_, CacheManager> {
        self.cache_manager.lock().unwrap()
    }
}
```

### Service Configuration

For configurable services, consider:
- Environment-based configuration during initialization
- Configuration services providing settings to other services
- Builder patterns for complex service setup

## Testing Considerations

### Unit Testing

Services can be tested independently of the registry:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let registry = ServiceRegistry::new();
        let manager = registry.notification_manager();
        assert_eq!(manager.subscriber_count(), 0);
    }
}
```

### Integration Testing

The global registry can be used in integration tests:

```rust
#[test]
fn test_global_access() {
    let services = get_services();
    let manager1 = services.notification_manager();
    let count1 = manager1.subscriber_count();
    drop(manager1);

    let manager2 = services.notification_manager();
    let count2 = manager2.subscriber_count();

    // Same underlying service
    assert_eq!(count1, count2);
}
```

### Concurrent Testing

Test thread safety:

```rust
#[test]
fn test_concurrent_access() {
    let handles: Vec<_> = (0..10).map(|_| {
        std::thread::spawn(|| {
            let services = get_services();
            let _manager = services.notification_manager();
            // Simulate work
            std::thread::sleep(std::time::Duration::from_millis(10));
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap();
    }
    // Test passes if no deadlocks or panics occur
}
```

## Best Practices

### Service Access
1. **Short Critical Sections**: Hold `MutexGuard`s for minimal time
2. **Avoid Nested Locks**: Don't acquire multiple service locks simultaneously
3. **Early Release**: Drop guards explicitly when done with services

### Error Handling
1. **Service Errors**: Handle service-specific errors appropriately
2. **Panic Safety**: Ensure service methods don't panic unexpectedly
3. **Resource Cleanup**: Services should clean up properly on drop

### Performance
1. **Batch Operations**: Group multiple operations under single lock
2. **Read-Heavy Services**: Consider `RwLock` for services with many readers
3. **Lock-Free Paths**: Use atomic operations where possible within services