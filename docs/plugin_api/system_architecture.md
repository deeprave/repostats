# System Architecture Documentation for Plugin Authors

## Overview

The repostats system provides a sophisticated plugin architecture built around four core components: **Plugin Manager**, **Queue System**, **Scanner Integration**, and **Notification System**. Understanding how these components work together is essential for plugin authors to build effective and well-integrated plugins.

## Architecture Overview

### Data Flow

```
Repository → Scanner → Queue → Analysis Plugins → Output Plugins → Reports
                    ↓
              Notification System (Events for all components)
```

### Core Components

1. **Plugin Manager**: Discovers, registers, and manages plugin lifecycle
2. **Queue System**: Provides message-passing infrastructure between scanner and plugins
3. **Scanner Integration**: Generates repository data based on plugin requirements
4. **Notification System**: Provides event-driven communication across the system

## Plugin Manager

The Plugin Manager serves as the central orchestrator for all plugin operations, handling discovery, activation, and lifecycle management.

### Plugin Lifecycle

#### 1. Discovery Phase

The Plugin Manager discovers plugins through multiple sources:

```rust
// Built-in plugins (compiled into the application)
registry.register_builtin_plugin("dump", || Box::new(DumpPlugin::new()));

// External plugins (shared libraries)
// Discovery happens in configured plugin directories
```

#### 2. Registration Phase

During registration, plugins provide metadata through `PluginInfo`:

```rust
fn plugin_info(&self) -> PluginInfo {
    PluginInfo {
        name: "example".to_string(),
        version: "1.0.0".to_string(),
        description: "Example analysis plugin".to_string(),
        author: "Plugin Author".to_string(),
        api_version: 20250101,
        plugin_type: PluginType::Processing,  // Determines queue access
        functions: vec![/* CLI functions */],
        required: ScanRequires::FILE_CHANGES.bits(),  // Scanner requirements
        auto_active: false,  // Manual activation required
    }
}
```

#### 3. Activation Phase

Plugins are activated in two ways:

**Manual Activation** (via CLI commands):
```bash
repostats analyse --since 1week
# Activates plugin with "analyse" function, args: ["--since", "1week"]
```

**Auto Activation** (during discovery):
```rust
PluginInfo {
    auto_active: true,  // Plugin activates automatically
    // ...
}
```

#### 4. Initialization Phase

Once activated, plugins go through initialization:

```rust
impl Plugin for MyPlugin {
    async fn initialize(&mut self) -> PluginResult<()> {
        // Set up internal state
        // Validate configuration
        // Prepare for execution
        Ok(())
    }

    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()> {
        // Parse CLI arguments
        // Apply TOML configuration
        // Validate arguments
        Ok(())
    }
}
```

### Plugin Types and Queue Access

Plugin type determines queue subscriber allocation:

```rust
pub enum PluginType {
    Processing,     // Gets QueueConsumer - processes messages real-time
    Output,         // No QueueConsumer - works with processed data
    Notification,   // No QueueConsumer - responds to events only
}
```

**Processing Plugins** (with QueueConsumer):
- Receive scan messages in real-time
- Process, transform, or analyze data
- Examples: indexers, pattern detectors, statistics calculators

**Output Plugins** (no QueueConsumer):
- Generate final reports or exports
- Work with data from Processing plugins
- Examples: CSV exporters, report generators

**Notification Plugins** (no QueueConsumer):
- Respond to system events
- Handle monitoring and alerting
- Examples: webhook notifiers, health monitors

### Plugin Configuration

Plugins receive configuration through two channels:

#### CLI Arguments
```rust
async fn parse_plugin_arguments(
    &mut self,
    args: &[String],           // ["--format", "json", "--verbose"]
    config: &PluginConfig,
) -> PluginResult<()> {
    // Parse using PluginSettings (simple) or PluginArgParser (clap)
}
```

#### TOML Configuration
```toml
# repostats.toml
[my_plugin]
default_format = "json"
max_entries = 1000
verbose = true
```

```rust
// In plugin
let format = config.get_string("default_format", "text");
let max_entries = config.get_string("max_entries", "100")
    .parse::<usize>()
    .unwrap_or(100);
let verbose = config.get_bool("verbose", false);
```

## Queue System

The Queue System provides a single global message queue that all producers (scanners) publish to and all consumers (plugins) read from.

### Queue Architecture

```
Scanner(s) → Global Queue → Plugin Consumer 1
                        → Plugin Consumer 2
                        → Plugin Consumer N
```

### Message Consumption for Processing Plugins

Processing plugins automatically receive a `QueueConsumer` and implement the `ConsumerPlugin` trait:

```rust
#[async_trait::async_trait]
impl ConsumerPlugin for MyPlugin {
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        // Store the consumer
        self.consumer = Some(consumer);

        // Spawn background task for message processing
        let consumer_clone = self.consumer.as_ref().unwrap().clone();
        tokio::spawn(async move {
            loop {
                match consumer_clone.receive_message().await {
                    Ok(message) => {
                        // Process the message
                        self.process_scan_message(message).await?;
                    }
                    Err(e) => {
                        log::error!("Failed to receive message: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    async fn stop_consuming(&mut self) -> PluginResult<()> {
        // Clean shutdown of consumer
        if let Some(consumer) = &self.consumer {
            consumer.close().await?;
        }
        self.consumer = None;
        Ok(())
    }
}
```

### Message Processing

Plugins receive `ScanMessage` enum variants based on their requirements:

```rust
use crate::scanner::types::ScanMessage;

async fn process_scan_message(&mut self, message: ScanMessage) -> PluginResult<()> {
    match message {
        ScanMessage::RepositoryData { scanner_id, timestamp, repository_data } => {
            // Process repository metadata
            println!("Repository: {} at {}", repository_data.name, repository_data.path);
        }
        ScanMessage::CommitData { scanner_id, timestamp, commit_info } => {
            // Process individual commits
            println!("Commit: {} by {}", commit_info.hash, commit_info.author_name);
        }
        ScanMessage::FileChange { scanner_id, timestamp, file_change } => {
            // Process file changes within commits
            println!("File changed: {} ({})", file_change.path, file_change.change_type);
        }
        ScanMessage::ScanCompleted { scanner_id, timestamp } => {
            // Scanner finished - finalize processing
            self.finalize_analysis().await?;
        }
        ScanMessage::ScanError { scanner_id, timestamp, error_message } => {
            // Handle scanner errors
            log::error!("Scanner error: {}", error_message);
        }
    }
    Ok(())
}
```

### Queue Consumer Management

The Plugin Manager handles consumer lifecycle:

1. **Creation**: Consumers created during plugin activation for Processing plugins
2. **Activation**: Consumers activated after plugin initialization
3. **Management**: Plugin Manager monitors consumer health and lag
4. **Cleanup**: Consumers cleaned up during plugin shutdown

```rust
// Plugin Manager creates consumers for Processing plugins
if plugin_type == PluginType::Processing {
    let consumer = queue_manager.create_consumer(plugin_name.clone())?;
    pending_consumers.insert(plugin_name, consumer);
}

// Later activation
for (plugin_name, consumer) in pending_consumers.drain() {
    if let Some(plugin) = registry.get_consumer_plugin_mut(&plugin_name) {
        plugin.start_consuming(consumer).await?;
    }
}
```

### Message Types and Content

Messages contain different data based on scanner requirements:

#### RepositoryData Message
```rust
ScanMessage::RepositoryData {
    scanner_id,     // Unique scanner identifier
    timestamp,      // When message was created
    repository_data: RepositoryData {
        name: String,           // Repository name
        path: String,           // Local path
        current_branch: String, // Active branch
        remote_url: Option<String>, // Git remote
        // ... additional metadata
    }
}
```

#### CommitData Message
```rust
ScanMessage::CommitData {
    scanner_id,
    timestamp,
    commit_info: CommitInfo {
        hash: String,           // Full commit hash
        short_hash: String,     // Abbreviated hash
        author_name: String,    // Commit author
        author_email: String,   // Author email
        committer_name: String, // Committer (may differ)
        committer_email: String,
        commit_time: SystemTime,
        message: String,        // Commit message
        parent_hashes: Vec<String>, // Parent commits
        // ... additional commit data
    }
}
```

#### FileChange Message
```rust
ScanMessage::FileChange {
    scanner_id,
    timestamp,
    file_change: FileChangeData {
        commit_hash: String,    // Associated commit
        path: String,           // File path
        old_path: Option<String>, // For renames
        change_type: ChangeType,  // Added/Modified/Deleted/Renamed/Copied
        lines_added: u32,       // Diff statistics
        lines_removed: u32,
        file_content: Option<String>, // If FILE_CONTENT required
        // ... additional change data
    }
}
```

## Scanner Integration

The Scanner system generates repository data based on the combined requirements of all active plugins.

### Requirement Declaration

Plugins declare their data needs through `ScanRequires`:

```rust
impl Plugin for MyPlugin {
    fn requirements(&self) -> ScanRequires {
        // Declare what data this plugin needs
        ScanRequires::FILE_CHANGES | ScanRequires::REPOSITORY_INFO
        // Dependencies (COMMITS) included automatically
    }
}
```

### Requirement Aggregation

The Plugin Manager aggregates requirements from all active plugins:

```rust
// Plugin Manager collects all requirements
let mut combined_requirements = ScanRequires::NONE;
for active_plugin in &self.active_plugins {
    if let Some(plugin) = registry.get_plugin(&active_plugin.plugin_name) {
        combined_requirements = combined_requirements.union(plugin.requirements());
    }
}

// Scanner receives combined requirements
scanner.set_requirements(combined_requirements);
```

### Scanner Optimization

Scanner only generates data that plugins actually need:

```rust
// If no plugins need FILE_CONTENT, scanner skips file reading
if !requirements.requires_file_content() {
    // Skip expensive file I/O operations
    return ScanMessage::FileChange {
        file_change: FileChangeData {
            file_content: None,  // No content provided
            // ... other fields
        }
    };
}
```

### Available Requirements

#### `ScanRequires::REPOSITORY_INFO`
- Repository metadata (name, path, branch, remote)
- Lightweight, minimal performance impact
- Useful for plugins that need context

#### `ScanRequires::COMMITS`
- Individual commit information (hash, author, message, timestamp)
- Includes parent relationships for merge analysis
- Medium performance impact

#### `ScanRequires::FILE_CHANGES`
- File modification details for each commit
- Change types, paths, diff statistics
- Automatically includes `COMMITS`
- Higher performance impact

#### `ScanRequires::FILE_CONTENT`
- Actual file content at each commit/HEAD
- Most expensive requirement
- Automatically includes `FILE_CHANGES` and `COMMITS`
- Use sparingly for static analysis plugins

#### `ScanRequires::HISTORY`
- Full commit history traversal
- Enables historical trend analysis
- Automatically includes `COMMITS`
- Performance depends on repository size

### Usage Patterns

**Basic Repository Analysis**:
```rust
fn requirements(&self) -> ScanRequires {
    ScanRequires::REPOSITORY_INFO
}
```

**Commit Pattern Analysis**:
```rust
fn requirements(&self) -> ScanRequires {
    ScanRequires::COMMITS  // Gets commit info only
}
```

**Code Change Analysis**:
```rust
fn requirements(&self) -> ScanRequires {
    ScanRequires::FILE_CHANGES  // Gets commits + file changes
}
```

**Static Code Analysis**:
```rust
fn requirements(&self) -> ScanRequires {
    ScanRequires::FILE_CONTENT  // Gets commits + changes + content
}
```

**Historical Trend Analysis**:
```rust
fn requirements(&self) -> ScanRequires {
    ScanRequires::HISTORY | ScanRequires::REPOSITORY_INFO
}
```

## Notification System

The Notification System provides event-driven communication across all system components.

### Event Types

The system publishes various event types:

```rust
pub enum Event {
    System(SystemEvent),    // System lifecycle events
    Queue(QueueEvent),      // Queue operations
    Scan(ScanEvent),        // Scanner events
    Plugin(PluginEvent),    // Plugin events
}
```

### System Events
```rust
pub enum SystemEventType {
    Started,        // System initialization complete
    Shutdown,       // System shutting down
    ConfigReload,   // Configuration reloaded
}
```

### Queue Events
```rust
pub enum QueueEventType {
    Started,        // Queue manager started
    PublisherCreated,   // New publisher created
    ConsumerCreated,    // New consumer created
    MessagePublished,   // Message published to queue
    MemoryPressure,     // Memory threshold exceeded
}
```

### Scanner Events
```rust
pub enum ScanEventType {
    Started,        // Scanner started
    Progress,       // Scanning progress update
    Completed,      // Scan completed successfully
    Error,          // Scanner error occurred
}
```

### Plugin Events
```rust
pub enum PluginEventType {
    Discovered,     // Plugin discovered
    Registered,     // Plugin registered
    Activated,      // Plugin activated
    Deactivated,    // Plugin deactivated
    Error,          // Plugin error occurred
}
```

### Subscribing to Events

Processing plugins automatically get notification subscribers:

```rust
// Plugin Manager creates notification subscribers for active plugins
for active_plugin in &self.active_plugins {
    let subscriber_id = format!("plugin-{}-notifications", active_plugin.plugin_name);
    let receiver = notification_manager.subscribe(
        subscriber_id,
        EventFilter::All,  // Receive all event types
        format!("Plugin-{}", active_plugin.plugin_name)
    )?;

    // Store receiver to keep channel alive
    self.notification_receivers.insert(subscriber_id, receiver);
}
```

### Event Processing in Plugins

Plugins can process events for coordination and monitoring:

```rust
impl Plugin for MyPlugin {
    async fn initialize(&mut self) -> PluginResult<()> {
        // Get notification receiver from plugin manager
        // or create a custom subscriber

        let services = get_services();
        let mut notification_manager = services.notification_manager().await;
        let receiver = notification_manager.subscribe(
            "my-plugin-events".to_string(),
            EventFilter::SystemOnly,  // Only system events
            "MyPlugin".to_string()
        )?;

        // Spawn task to handle events
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                match event {
                    Event::System(system_event) => {
                        match system_event.event_type {
                            SystemEventType::Shutdown => {
                                // Prepare for shutdown
                                self.prepare_shutdown().await;
                            }
                            _ => {}
                        }
                    }
                    Event::Scan(scan_event) => {
                        match scan_event.event_type {
                            ScanEventType::Completed => {
                                // Scanning finished - can now generate final reports
                                self.generate_final_report().await;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }
}
```

### Event Filtering

Plugins can subscribe to specific event types:

```rust
use crate::notifications::api::{EventFilter, Event};

// Subscribe to specific event types
let receiver = notification_manager.subscribe(
    "my-plugin".to_string(),
    EventFilter::ScanOnly,    // Only scanner events
    "MyPlugin".to_string()
)?;

// Available filters:
// EventFilter::All          - All event types
// EventFilter::SystemOnly   - System events only
// EventFilter::QueueOnly    - Queue events only
// EventFilter::ScanOnly     - Scanner events only
// EventFilter::PluginOnly   - Plugin events only
```

## Complete Plugin Example

Here's a comprehensive example showing how all components work together:

```rust
use crate::plugin::traits::{Plugin, ConsumerPlugin};
use crate::plugin::types::{PluginInfo, PluginType, PluginFunction};
use crate::scanner::types::{ScanRequires, ScanMessage};
use crate::queue::api::QueueConsumer;
use crate::plugin::args::PluginConfig;
use crate::notifications::api::{Event, EventFilter};
use std::collections::HashMap;

pub struct AnalysisPlugin {
    // Plugin state
    initialized: bool,
    consumer: Option<QueueConsumer>,

    // Analysis data
    commit_count: u64,
    file_changes: HashMap<String, u32>,

    // Configuration
    verbose: bool,
    output_format: String,
}

impl AnalysisPlugin {
    pub fn new() -> Self {
        Self {
            initialized: false,
            consumer: None,
            commit_count: 0,
            file_changes: HashMap::new(),
            verbose: false,
            output_format: "text".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Plugin for AnalysisPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: "analysis".to_string(),
            version: "1.0.0".to_string(),
            description: "Repository analysis plugin".to_string(),
            author: "Plugin Author".to_string(),
            api_version: 20250101,
            plugin_type: PluginType::Processing,  // Gets queue consumer
            functions: vec![
                PluginFunction {
                    name: "analyse".to_string(),
                    description: "Analyse repository patterns".to_string(),
                    aliases: vec!["analyze".to_string(), "stats".to_string()],
                }
            ],
            required: (ScanRequires::FILE_CHANGES | ScanRequires::REPOSITORY_INFO).bits(),
            auto_active: false,
        }
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Processing
    }

    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![
            PluginFunction {
                name: "analyse".to_string(),
                description: "Analyse repository patterns".to_string(),
                aliases: vec!["analyze".to_string(), "stats".to_string()],
            }
        ]
    }

    fn requirements(&self) -> ScanRequires {
        ScanRequires::FILE_CHANGES | ScanRequires::REPOSITORY_INFO
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        if self.verbose {
            println!("Initializing analysis plugin...");
        }
        self.initialized = true;
        Ok(())
    }

    async fn execute(&mut self, _args: &[String]) -> PluginResult<()> {
        // Direct execution (if needed)
        if !self.initialized {
            return Err(PluginError::ExecutionError {
                plugin_name: "analysis".to_string(),
                operation: "execute".to_string(),
                cause: "Plugin not initialized".to_string(),
            });
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        if self.verbose {
            println!("Analysis plugin cleanup - processed {} commits", self.commit_count);
        }
        Ok(())
    }

    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()> {
        // Parse CLI arguments using simple parser
        let settings = PluginSettings::from_cli_args("analyse".to_string(), args.to_vec())?;

        // Apply settings
        self.verbose = settings.has_flag("verbose") || config.get_bool("verbose", false);
        self.output_format = settings.get_arg("format")
            .cloned()
            .unwrap_or_else(|| config.get_string("output_format", "text"));

        if self.verbose {
            println!("Analysis plugin configured: format={}, verbose={}",
                    self.output_format, self.verbose);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ConsumerPlugin for AnalysisPlugin {
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        if self.verbose {
            println!("Starting message consumption...");
        }

        self.consumer = Some(consumer.clone());

        // Spawn background task for message processing
        let verbose = self.verbose;
        tokio::spawn(async move {
            loop {
                match consumer.receive_message().await {
                    Ok(message) => {
                        if let Err(e) = Self::process_message(message, verbose).await {
                            log::error!("Error processing message: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to receive message: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    async fn stop_consuming(&mut self) -> PluginResult<()> {
        if let Some(consumer) = &self.consumer {
            consumer.close().await?;
        }
        self.consumer = None;

        if self.verbose {
            println!("Stopped consuming messages");
        }
        Ok(())
    }
}

impl AnalysisPlugin {
    async fn process_message(message: ScanMessage, verbose: bool) -> PluginResult<()> {
        match message {
            ScanMessage::RepositoryData { repository_data, .. } => {
                if verbose {
                    println!("Processing repository: {}", repository_data.name);
                }
            }
            ScanMessage::CommitData { commit_info, .. } => {
                if verbose {
                    println!("Processing commit: {} by {}",
                            commit_info.short_hash, commit_info.author_name);
                }
                // Increment commit counter (would need shared state in real implementation)
            }
            ScanMessage::FileChange { file_change, .. } => {
                if verbose {
                    println!("File changed: {} ({})",
                            file_change.path, file_change.change_type);
                }
                // Track file changes (would need shared state in real implementation)
            }
            ScanMessage::ScanCompleted { .. } => {
                if verbose {
                    println!("Scan completed - generating analysis report");
                }
                // Generate final analysis report
            }
            ScanMessage::ScanError { error_message, .. } => {
                log::error!("Scanner error: {}", error_message);
            }
        }
        Ok(())
    }
}
```

## Best Practices

### Plugin Design

1. **Single Responsibility**: Each plugin should have a clear, focused purpose
2. **Resource Management**: Always clean up resources in `cleanup()`
3. **Error Handling**: Provide meaningful error messages with context
4. **Performance**: Consider memory usage and processing time
5. **Configuration**: Support both CLI and TOML configuration

### Queue Usage

1. **Processing Plugins**: Implement `ConsumerPlugin` for real-time processing
2. **Output Plugins**: Use `PluginType::Output` to avoid unnecessary consumers
3. **Message Handling**: Process messages efficiently to avoid queue backlog
4. **Error Recovery**: Handle message processing errors gracefully

### Scanner Requirements

1. **Minimal Requirements**: Only request data you actually need
2. **Performance Impact**: `FILE_CONTENT` is expensive - use judiciously
3. **Dependency Awareness**: Understand automatic requirement inclusion
4. **Testing**: Test with different requirement combinations

### Event Integration

1. **Lifecycle Events**: Use system events for coordination
2. **Scanner Events**: React to scan completion for final processing
3. **Error Events**: Handle errors from other components appropriately
4. **Subscription Management**: Keep event receivers alive as needed

## Common Patterns

### Data Collection and Analysis

```rust
// 1. Collect data during scan via queue messages
// 2. Process incrementally or batch at end
// 3. Generate reports when scan completes (via events)
```

### Multi-Stage Processing

```rust
// Processing Plugin (Stage 1) → Processing Plugin (Stage 2) → Output Plugin
// Use events to coordinate between stages
```

### Configuration-Driven Behavior

```rust
// Support both runtime (CLI) and persistent (TOML) configuration
// CLI overrides TOML overrides defaults
```

### Resource Monitoring

```rust
// Monitor queue lag, memory usage via system APIs
// Implement backpressure and cleanup as needed
```

## See Also

- [Plugin Functions Documentation](plugin_functions.md)
- [CLI Integration Documentation](cli_integration.md)
- [ScanRequires Documentation](scan_requires.md)
- [PluginInfo Documentation](plugin_info.md)
- [Plugin Author Guide](plugin_author_guide.md)
