# Streaming Scanner Implementation

## Overview

The scanner system uses a streaming callback pattern to handle large repositories efficiently without accumulating messages in memory. This document describes the implementation, performance characteristics, and usage patterns.

## Architecture

### Streaming Callback Pattern

The core scanning method `scan_commits_with_query()` uses a callback pattern where messages are processed immediately as they're generated:

```rust
pub async fn scan_commits_with_query<F>(
    &self,
    query_params: Option<&QueryParams>,
    mut message_handler: F,
) -> ScanResult<()>
where
    F: FnMut(ScanMessage) -> ScanResult<()>,
```

### Message Flow

Messages are emitted in a predictable sequence:

1. **RepositoryData** - Repository metadata (first message)
2. **CommitData** - Individual commit information (one per matching commit)
3. **ScanCompleted** - Successful scan completion marker (final message)

### Error Handling

- If the callback returns an error, scanning stops immediately
- Errors are propagated up to the caller
- This enables fail-fast behaviour and backpressure control

## Performance Characteristics

### Memory Efficiency

- **No message accumulation**: Messages are processed immediately via callback
- **Bounded memory usage**: Memory usage is constant regardless of repository size
- **Streaming processing**: One commit processed at a time

### Scalability

- **Large repository support**: Can handle repositories with thousands of commits
- **Memory safety**: Prevents out-of-memory issues on large scans
- **Async-friendly**: Non-blocking operation using Rust's async runtime

## Implementation Details

### Publishing Integration

The streaming scanner integrates with two publishing approaches:

1. **Immediate Publishing** (`scan_commits_and_publish_incrementally_with_query`)
   - Each message published immediately to queue
   - True streaming with no intermediate buffering
   - Used for production workloads

2. **Collection Pattern** (test helpers)
   - Messages collected into Vec for testing
   - Used in test scenarios only

### Performance Optimisations

- **Pre-compiled patterns**: Author filters compiled once and reused
- **Efficient timestamp conversion**: Helper method prevents code duplication
- **Minimal allocations**: String operations optimised to avoid repeated allocations

## Usage Patterns

### Production Usage

```rust
// Immediate publishing to queue
scanner_task.scan_commits_and_publish_incrementally_with_query(query_params).await?;
```

### Testing Usage

```rust
// Collect messages for testing
let mut messages = Vec::new();
scanner_task.scan_commits_with_query(query_params, |msg| {
    messages.push(msg);
    Ok(())
}).await?;
```

### Custom Processing

```rust
// Custom message handling with error propagation
scanner_task.scan_commits_with_query(query_params, |msg| {
    match msg {
        ScanMessage::CommitData { commit_info, .. } => {
            // Process commit
            process_commit(&commit_info)?;
        }
        _ => {
            // Handle other message types
        }
    }
    Ok(())
}).await?;
```

## Future Considerations

### File Change Batching

When file diff parsing is implemented:
- File changes will be batched per commit
- Commit and its file changes published as atomic unit
- Maintains streaming at commit level while batching file changes

### Configuration

Future enhancements may include:
- Configurable batch sizes for different repository sizes
- Adaptive batching based on commit complexity
- Memory pressure monitoring and backpressure

## Monitoring and Debugging

### Error Scenarios

- **Callback errors**: Stop scanning and propagate error
- **Git repository errors**: Wrapped in `ScanError::Repository`
- **Serialisation errors**: Wrapped in `ScanError::Io`

### Performance Monitoring

The system provides scan statistics including:
- Total commits processed
- Scan duration
- Memory usage patterns (via system monitoring)

## Related Documentation

- [Multi Consumer Queue](multiconsumer_queue.md) - Queue system integration
- [Service Registry](service_registry.md) - Service integration patterns
- [API Versioning](api_versioning.md) - Message format evolution
