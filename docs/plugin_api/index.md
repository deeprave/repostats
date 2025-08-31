# Plugin API Documentation

## Overview

### What is a Plugin?

In the repostats system, a **plugin** is an external component that extends the core functionality of the repository analysis tool. Plugins are dynamically loadable shared libraries (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows) written in Rust that integrate seamlessly with the repostats pipeline.

Plugins operate within a sophisticated data processing pipeline:

```
Repository → Scanner → Queue → Processing Plugins → Output Plugins → Reports/Exports
                    ↓
              Notification System (Events)
```

### What Can Plugins Be Used For?

Plugins enable extensible repository analysis through various types of functionality:

#### **Processing Plugins** (Real-time Data Processing)
- **Code Analysis**: Static analysis, complexity metrics, code quality assessment
- **Pattern Detection**: Security vulnerabilities, coding standards violations, architectural patterns
- **Data Indexing**: Building searchable indexes of code, commits, or files
- **Statistics Collection**: Gathering metrics on commits, contributors, file changes
- **Monitoring**: Tracking of repository activity over time and overall health

#### **Output Plugins** (Report Generation)
- **Export Formats**: Text, CSV, JSON, XML, Excel, PDF report generation
- **Visualization**: Charts, graphs, dependency diagrams
- **Integration**: Pushing data to external systems (databases, APIs, dashboards and graphing tools)
- **Documentation**: Generating project documentation from code analysis
- **Compliance**: Regulatory reporting and audit trail generation

#### **Notification Plugins** (Potential Event-Driven Actions)
- **Alerting**: Email, Slack, webhook notifications for important events
- **Integration**: Triggering CI/CD pipelines, updating project management tools
- **Monitoring**: System health checks, performance monitoring
- **Automation**: Automated responses to repository changes or analysis results

### Plugin Requirements

#### Generic Requirements (All Plugins)

1. **Rust cdylib Library**: Must be compiled as a dynamic library
2. **API Compatibility**: Must implement the current plugin API version (20250725)
3. **YAML Manifest**: Must include a `plugin.yaml` file with metadata
4. **Plugin Trait Implementation**: Must implement the `Plugin` trait
5. **Thread Safety**: Must be `Send + Sync` for concurrent execution
6. **Error Handling**: Must use the plugin result system for error reporting

#### Specific Requirements by Type

**Processing Plugins:**
- Must implement `ConsumerPlugin` trait for queue message processing
- Should declare `ScanRequires` for selective scanner data needs
- Must handle `ScanMessage` variants appropriately
- Should be designed for repository data processing

**Output Plugins:**
- Should generate final reports or exports
- May work with processed data from Processing plugins
- Does not need queue consumers
- Should support various output formats and integrations

**Notification Plugins:**
- Should implement event subscribers for system notifications
- Must handle `Event` types from the notification system
- May need external service integration

## Table of Contents

### Core API Documentation

#### [PluginInfo Documentation](plugin_info.md)
**Purpose**: Complete reference for plugin metadata structure
**Contains**: Field descriptions, validation rules, API version requirements
**Use When**: Setting up plugin metadata, understanding plugin registration process

#### [ScanRequires Documentation](scan_requires.md)
**Purpose**: Data requirements system for scanner optimization
**Contains**: Bitflag system, dependency rules, performance implications
**Use When**: Declaring what repository data your plugin needs from the scanner

#### [Plugin Functions Documentation](plugin_functions.md)
**Purpose**: Multi-interface command system for CLI integration
**Contains**: Function definitions, aliases, command matching
**Use When**: Implementing CLI commands and function routing in your plugin

#### [CLI Integration Documentation](cli_integration.md)
**Purpose**: Complete guide to command-line argument processing
**Contains**: Argument parsing, configuration, segmentation system
**Use When**: Implementing command-line argument handling and configuration support

#### [System Architecture Documentation](system_architecture.md)
**Purpose**: Comprehensive system integration guide
**Contains**: Plugin lifecycle, queue system, scanner integration, notification system
**Use When**: Understanding how plugins interact with the repostats system

## Creating an External Plugin

### Project Setup

#### 1. Create a New Rust Project

```bash
# Create a new Rust library project
cargo new --lib my_analysis_plugin
cd my_analysis_plugin
```

#### 2. Configure Cargo.toml

```toml
[package]
name = "my_analysis_plugin"
version = "1.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # Essential: Creates dynamic library

[dependencies]
# Core repostats dependencies
repostats = { path = "../repostats" }  # Adjust path as needed
async-trait = "0.1"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
log = "0.4"

# Additional dependencies as needed
anyhow = "1.0"
```

#### 3. Create Plugin Manifest (plugin.yaml)

```yaml
# plugin.yaml - Must be in the same directory as your compiled library
name: "my-analysis-plugin"
version: "1.0.0"
api_version: "20250727"
description: "Custom repository analysis plugin"
author: "Your Name <your.email@example.com>"
license: "MIT"

library:
  name: "libmy_analysis_plugin"
  # Platform-specific extensions added automatically:
  # Linux: libmy_analysis_plugin.so
  # macOS: libmy_analysis_plugin.dylib
  # Windows: my_analysis_plugin.dll

commands:
  - "analyze"
  - "report"
  - "export"

dependencies:
  min_rust_version: "1.80"
  required_features: ["tokio", "serde"]

metadata:
  homepage: "https://github.com/yourname/my-analysis-plugin"
  repository: "https://github.com/yourname/my-analysis-plugin"
  documentation: "https://docs.rs/my-analysis-plugin"
```

### Plugin Implementation

#### 4. Basic Plugin Structure (src/lib.rs)

```rust
use repostats::plugin::traits::{Plugin, ConsumerPlugin};
use repostats::plugin::types::{PluginInfo, PluginType, PluginFunction};
use repostats::plugin::args::PluginConfig;
use repostats::plugin::error::PluginResult;
use repostats::scanner::types::{ScanRequires, ScanMessage};
use repostats::queue::api::QueueConsumer;
use std::collections::HashMap;

/// Your custom analysis plugin
pub struct MyAnalysisPlugin {
    initialized: bool,
    consumer: Option<QueueConsumer>,

    // Plugin-specific state
    commit_count: u64,
    file_changes: HashMap<String, u32>,

    // Configuration
    output_format: String,
    verbose: bool,
}

impl MyAnalysisPlugin {
    pub fn new() -> Self {
        Self {
            initialized: false,
            consumer: None,
            commit_count: 0,
            file_changes: HashMap::new(),
            output_format: "json".to_string(),
            verbose: false,
        }
    }

    async fn process_scan_message(&mut self, message: ScanMessage) -> PluginResult<()> {
        match message {
            ScanMessage::RepositoryData { repository_data, .. } => {
                if self.verbose {
                    println!("Analyzing repository: {}", repository_data.name);
                }
                // Process repository metadata
            }
            ScanMessage::CommitData { commit_info, .. } => {
                self.commit_count += 1;
                if self.verbose {
                    println!("Processing commit: {}", commit_info.short_hash);
                }
                // Analyze commit data
            }
            ScanMessage::FileChange { file_change, .. } => {
                let counter = self.file_changes.entry(file_change.path.clone()).or_insert(0);
                *counter += 1;
                // Track file change patterns
            }
            ScanMessage::ScanCompleted { .. } => {
                // Generate final analysis report
                self.generate_report().await?;
            }
            ScanMessage::ScanError { error_message, .. } => {
                log::error!("Scanner error: {}", error_message);
            }
        }
        Ok(())
    }

    async fn generate_report(&self) -> PluginResult<()> {
        match self.output_format.as_str() {
            "json" => {
                let report = serde_json::json!({
                    "total_commits": self.commit_count,
                    "files_changed": self.file_changes.len(),
                    "most_changed_files": self.get_most_changed_files()
                });
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            }
            "text" => {
                println!("=== Analysis Report ===");
                println!("Total commits: {}", self.commit_count);
                println!("Files changed: {}", self.file_changes.len());
                // Add more text output...
            }
            _ => {
                log::warn!("Unsupported output format: {}", self.output_format);
            }
        }
        Ok(())
    }

    fn get_most_changed_files(&self) -> Vec<(String, u32)> {
        let mut files: Vec<_> = self.file_changes.iter()
            .map(|(path, count)| (path.clone(), *count))
            .collect();
        files.sort_by(|a, b| b.1.cmp(&a.1));
        files.into_iter().take(10).collect()
    }
}

#[async_trait::async_trait]
impl Plugin for MyAnalysisPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: "my-analysis-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Custom repository analysis plugin".to_string(),
            author: "Your Name <your.email@example.com>".to_string(),
            api_version: repostats::get_plugin_api_version(),
            plugin_type: PluginType::Processing,  // Gets queue consumer
            functions: vec![
                PluginFunction {
                    name: "analyze".to_string(),
                    description: "Perform repository analysis".to_string(),
                    aliases: vec!["analyse".to_string(), "scan".to_string()],
                },
                PluginFunction {
                    name: "report".to_string(),
                    description: "Generate analysis report".to_string(),
                    aliases: vec!["summary".to_string()],
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
                name: "analyze".to_string(),
                description: "Perform repository analysis".to_string(),
                aliases: vec!["analyse".to_string(), "scan".to_string()],
            },
            PluginFunction {
                name: "report".to_string(),
                description: "Generate analysis report".to_string(),
                aliases: vec!["summary".to_string()],
            }
        ]
    }

    fn requirements(&self) -> ScanRequires {
        ScanRequires::FILE_CHANGES | ScanRequires::REPOSITORY_INFO
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        println!("Initializing My Analysis Plugin v1.0.0");
        self.initialized = true;
        Ok(())
    }

    async fn execute(&mut self, _args: &[String]) -> PluginResult<()> {
        if !self.initialized {
            return Err(repostats::plugin::error::PluginError::ExecutionError {
                plugin_name: "my-analysis-plugin".to_string(),
                operation: "execute".to_string(),
                cause: "Plugin not initialized".to_string(),
            });
        }
        println!("Plugin executed successfully");
        Ok(())
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        println!("My Analysis Plugin cleanup completed");
        Ok(())
    }

    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()> {
        use repostats::plugin::settings::PluginSettings;

        // Parse CLI arguments
        let settings = PluginSettings::from_cli_args("analyze".to_string(), args.to_vec())?;

        // Apply configuration
        self.output_format = settings.get_arg("format")
            .cloned()
            .unwrap_or_else(|| config.get_string("output_format", "json"));

        self.verbose = settings.has_flag("verbose") || config.get_bool("verbose", false);

        if self.verbose {
            println!("Plugin configured: format={}, verbose={}", self.output_format, self.verbose);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ConsumerPlugin for MyAnalysisPlugin {
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        println!("Starting message consumption...");
        self.consumer = Some(consumer.clone());

        // Clone necessary data for the async task
        let verbose = self.verbose;

        // Spawn background task for message processing
        tokio::spawn(async move {
            loop {
                match consumer.receive_message().await {
                    Ok(message) => {
                        // Process message (this is simplified - in reality you'd need
                        // to pass the message back to the plugin instance)
                        if verbose {
                            println!("Received message: {:?}", message.message_type());
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
        println!("Stopped consuming messages");
        Ok(())
    }
}

/// Factory function required for dynamic loading
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn Plugin {
    Box::into_raw(Box::new(MyAnalysisPlugin::new()))
}

/// Plugin API version check - required for compatibility
#[no_mangle]
pub extern "C" fn plugin_api_version() -> u32 {
    20250101
}
```

### Building and Installation

#### 5. Build the Plugin

```bash
# Build the plugin as a release binary
cargo build --release

# The compiled library will be in target/release/
# Linux: libmy_analysis_plugin.so
# macOS: libmy_analysis_plugin.dylib
# Windows: my_analysis_plugin.dll
```

#### 6. Install the Plugin

```bash
# Create plugin directory structure
mkdir -p ~/.local/share/repostats/plugins/my-analysis-plugin

# Copy the compiled library and manifest
cp target/release/libmy_analysis_plugin.so ~/.local/share/repostats/plugins/my-analysis-plugin/
cp plugin.yaml ~/.local/share/repostats/plugins/my-analysis-plugin/

# The plugin should now be discoverable by repostats
repostats --plugins  # List available plugins
```

#### 7. Using the Plugin

```bash
# Use the plugin with CLI commands
repostats --config myconfig.toml analyze --format json --verbose

# The plugin will:
# 1. Be discovered via its manifest
# 2. Loaded as a dynamic library
# 3. Activated based on the "analyze" command
# 4. Process repository scan messages
# 5. Generate the final analysis report
```

### Development Tips

1. **Testing**: Create unit tests for your plugin logic, integration tests for the plugin interface
2. **Logging**: Use the `log` crate for proper logging integration
3. **Error Handling**: Always use `PluginResult` for error returns
4. **Performance**: Consider memory usage when processing large repositories
5. **Configuration**: Support both CLI arguments and TOML configuration files
6. **Documentation**: Document your plugin's functions, arguments, and configuration options

### Troubleshooting

- **Plugin Not Found**: Check that `plugin.yaml` exists alongside the compiled library
- **API Version Mismatch**: Ensure your plugin's API version matches the repostats version
- **Loading Errors**: Verify that all dependencies are available and the library is correctly compiled
- **Runtime Errors**: Check logs for detailed error messages and stack traces

## Next Steps

After reading this overview, explore the detailed documentation for specific aspects of plugin development:

1. Start with [System Architecture](system_architecture.md) to understand the overall system
2. Review [Plugin Functions](plugin_functions.md) to implement CLI commands
3. Study [CLI Integration](cli_integration.md) for argument processing
4. Reference [PluginInfo](plugin_info.md) and [ScanRequires](scan_requires.md) for API details

For questions or contributions, refer to the main repostats project documentation and issue tracker.
