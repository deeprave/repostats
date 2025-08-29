//! Dump Plugin - Output queue messages to stdout for debugging
//!
//! This plugin consumes messages from the global queue and outputs them in various formats
//! to stdout.
//! It's useful for debugging and monitoring message flow.

use crate::plugin::args::{
    create_format_args, determine_format, OutputFormat, PluginArgParser, PluginConfig,
};
use crate::plugin::traits::{ConsumerPlugin, Plugin, PluginFunction, PluginInfo, PluginType};
use crate::plugin::{PluginError, PluginResult};
use crate::queue::QueueConsumer;
use clap::Arg;
use log::{debug, error, info};
use serde_json::json;
use tokio::sync::oneshot;

/// Dump plugin for outputting queue messages to stdout
pub struct DumpPlugin {
    /// Plugin initialization state
    initialized: bool,

    /// Output format setting
    output_format: OutputFormat,

    /// Whether to show message headers
    show_headers: bool,

    /// Shutdown sender to signal task termination
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl DumpPlugin {
    /// Create a new dump plugin instance
    pub fn new() -> Self {
        Self {
            initialized: false,
            output_format: OutputFormat::Text,
            show_headers: true,
            shutdown_tx: None,
        }
    }
}

impl Default for DumpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DumpPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DumpPlu00gin")
            .field("initialized", &self.initialized)
            .field("output_format", &self.output_format)
            .field("show_headers", &self.show_headers)
            .field("shutdown_tx", &self.shutdown_tx.is_some())
            .finish()
    }
}

#[async_trait::async_trait]
impl Plugin for DumpPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: "dump".to_string(),
            version: "1.0.0".to_string(),
            description: "Output queue messages to stdout for debugging".to_string(),
            author: "RepoStats".to_string(),
            api_version: 20250101,
        }
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Output
    }

    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![PluginFunction {
            name: "dump".to_string(),
            description: "Start dumping messages to stdout".to_string(),
            aliases: vec!["start".to_string(), "run".to_string()],
        }]
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        debug!("DumpPlugin: Initializing");
        self.initialized = true;
        info!("DumpPlugin: Initialized successfully");
        Ok(())
    }

    async fn execute(&mut self, args: &[String]) -> PluginResult<()> {
        if !self.initialized {
            return Err(PluginError::ExecutionError {
                plugin_name: "dump".to_string(),
                operation: "execute".to_string(),
                cause: "Plugin not initialized".to_string(),
            });
        }

        debug!("DumpPlugin: Execute called with args: {:?}", args);

        // For now, just log that execute was called
        // The actual message consumption happens via ConsumerPlugin trait
        info!("DumpPlugin: Execute command received");
        Ok(())
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        debug!("DumpPlugin: Cleanup started");

        // Stop consuming first
        let _ = self.stop_consuming().await;

        self.initialized = false;
        info!("DumpPlugin: Cleanup completed");
        Ok(())
    }

    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()> {
        debug!("DumpPlugin: Parsing arguments: {:?}", args);

        // Create argument parser with format options and header control
        let plugin_info = self.plugin_info();
        let parser = PluginArgParser::new(
            &plugin_info.name,
            &plugin_info.description,
            &plugin_info.version,
        )
        .args(create_format_args())
        .arg(
            Arg::new("no-headers")
                .long("no-headers")
                .action(clap::ArgAction::SetTrue)
                .help("Don't show message headers (sequence, producer, etc.)"),
        );

        // Parse arguments using clap
        let matches = parser.parse(args)?;

        // Determine format from arguments and configuration
        self.output_format = determine_format(&matches, config);
        self.show_headers = !matches.get_flag("no-headers");

        debug!(
            "DumpPlugin: Arguments parsed - format={}, headers={}",
            self.output_format, self.show_headers
        );

        Ok(())
    }
}

#[async_trait::async_trait]
impl ConsumerPlugin for DumpPlugin {
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        debug!("DumpPlugin: Starting message consumption");

        // Create shutdown channel for graceful shutdown
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        // Capture plugin settings for the task
        let output_format = self.output_format;
        let show_headers = self.show_headers;

        // Spawn the consumer task that owns the consumer directly
        tokio::spawn(async move {
            let mut _message_count = 0;
            info!("DumpPlugin: Message consumption started");

            loop {
                tokio::select! {
                    // Check for shutdown signal
                    _ = &mut shutdown_rx => {
                        info!("DumpPlugin: Shutdown signal received, stopping message consumption");
                        break;
                    }

                    // Try to read a message (with timeout to allow shutdown checks)
                    result = tokio::time::timeout(
                        tokio::time::Duration::from_millis(100),
                        async { consumer.read() }
                    ) => {
                        match result {
                            Ok(Ok(Some(msg))) => {
                                // Format and output the message
                                let formatted = match output_format {
                                    OutputFormat::Json => {
                                        let json_obj = json!({
                                            "sequence": msg.header.sequence,
                                            "producer_id": msg.header.producer_id,
                                            "message_type": msg.header.message_type,
                                            "timestamp": msg.header
                                                .timestamp
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs(),
                                            "data": msg.data
                                        });
                                        json_obj.to_string()
                                    },
                                    OutputFormat::Compact => format!(
                                        "{}:{}:{}:{}",
                                        msg.header.sequence,
                                        msg.header.producer_id,
                                        msg.header.message_type,
                                        msg.data
                                    ),
                                    OutputFormat::Text => {
                                        if show_headers {
                                            format!(
                                                "[{}] {} from {}: {}",
                                                msg.header.sequence,
                                                msg.header.message_type,
                                                msg.header.producer_id,
                                                msg.data
                                            )
                                        } else {
                                            msg.data.clone()
                                        }
                                    }
                                };

                                println!("{}", formatted);
                                _message_count += 1;
                            }
                            Ok(Ok(None)) => {
                                // No messages available, continue the loop
                                // (timeout will naturally provide a small delay)
                            }
                            Ok(Err(e)) => {
                                error!("DumpPlugin: Error reading message: {:?}", e);
                                // Continue processing despite errors
                            }
                            Err(_) => {
                                // Timeout occurred, continue loop to check for shutdown
                            }
                        }
                    }
                }
            }

            info!("DumpPlugin: Message consumption stopped");
        });

        // Store the shutdown sender for lifecycle management
        self.shutdown_tx = Some(shutdown_tx);

        info!("DumpPlugin: Consumer task started");
        Ok(())
    }

    async fn stop_consuming(&mut self) -> PluginResult<()> {
        debug!("DumpPlugin: Stopping message consumption");

        // Send shutdown signal to the task
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            if let Err(_) = shutdown_tx.send(()) {
                debug!("DumpPlugin: Task may have already stopped");
            }
        }

        info!("DumpPlugin: Shutdown signal sent");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::QueueManager;

    #[tokio::test]
    async fn test_dump_plugin_creation() {
        let plugin = DumpPlugin::new();

        assert!(!plugin.initialized);
        assert_eq!(plugin.plugin_info().name, "dump");
        assert_eq!(plugin.plugin_info().version, "1.0.0");
        assert_eq!(plugin.plugin_type(), PluginType::Output);
    }

    #[tokio::test]
    async fn test_dump_plugin_default() {
        let plugin = DumpPlugin::default();

        assert!(!plugin.initialized);
        assert_eq!(plugin.output_format, OutputFormat::Text);
        assert!(plugin.show_headers);
    }

    #[tokio::test]
    async fn test_dump_plugin_initialization() {
        let mut plugin = DumpPlugin::new();

        // Should not be initialized initially
        assert!(!plugin.initialized);

        // Initialize the plugin
        let result = plugin.initialize().await;
        assert!(result.is_ok());
        assert!(plugin.initialized);

        // Cleanup
        let cleanup_result = plugin.cleanup().await;
        assert!(cleanup_result.is_ok());
        assert!(!plugin.initialized);
    }

    #[tokio::test]
    async fn test_dump_plugin_functions() {
        let plugin = DumpPlugin::new();
        let functions = plugin.advertised_functions();

        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "dump");
        assert!(functions[0].aliases.contains(&"start".to_string()));
        assert!(functions[0].aliases.contains(&"run".to_string()));
    }

    #[tokio::test]
    async fn test_dump_plugin_execute_requires_initialization() {
        let mut plugin = DumpPlugin::new();

        // Execute without initialization should fail
        let result = plugin.execute(&[]).await;
        assert!(result.is_err());

        // Initialize and try again
        let _ = plugin.initialize().await;
        let result = plugin.execute(&[]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_dump_plugin_argument_parsing() {
        let mut plugin = DumpPlugin::new();

        // Test JSON flag
        let config = PluginConfig::default();
        let result = plugin
            .parse_plugin_arguments(&["--json".to_string()], &config)
            .await;
        assert!(result.is_ok());
        assert_eq!(plugin.output_format, OutputFormat::Json);

        // Test no headers flag
        let result = plugin
            .parse_plugin_arguments(&["--no-headers".to_string()], &config)
            .await;
        assert!(result.is_ok());
        assert!(!plugin.show_headers);

        // Removed max-messages test as it's no longer supported

        // Test combined flags
        let result = plugin
            .parse_plugin_arguments(&["--json".to_string(), "--no-headers".to_string()], &config)
            .await;
        assert!(result.is_ok());
        assert_eq!(plugin.output_format, OutputFormat::Json);
        assert!(!plugin.show_headers);
    }

    #[tokio::test]
    async fn test_dump_plugin_argument_parsing_errors() {
        let mut plugin = DumpPlugin::new();
        let config = PluginConfig::default();

        // Test missing argument for max-messages
        let result = plugin
            .parse_plugin_arguments(&["--max-messages".to_string()], &config)
            .await;
        assert!(result.is_err());

        // Test invalid number for max-messages
        let result = plugin
            .parse_plugin_arguments(
                &["--max-messages".to_string(), "invalid".to_string()],
                &config,
            )
            .await;
        assert!(result.is_err());

        // Test unknown argument
        let result = plugin
            .parse_plugin_arguments(&["--unknown".to_string()], &config)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dump_plugin_help() {
        let mut plugin = DumpPlugin::new();
        let config = PluginConfig::default();

        // Test help flag
        let result = plugin
            .parse_plugin_arguments(&["--help".to_string()], &config)
            .await;
        assert!(result.is_err());

        if let Err(PluginError::Generic { message }) = result {
            // The help message should contain the plugin name and available options
            assert!(message.contains("dump"));
            assert!(message.contains("--json"));
            assert!(message.contains("--no-headers"));
        } else {
            panic!("Expected Generic error with help message");
        }

        // Test short help flag
        let result = plugin
            .parse_plugin_arguments(&["-h".to_string()], &config)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dump_plugin_consumer_lifecycle() {
        let mut plugin = DumpPlugin::new();
        let queue_manager = QueueManager::create().await;

        // Initialize plugin first
        let _ = plugin.initialize().await;

        // Create consumer
        let consumer = queue_manager
            .create_consumer("dump-test".to_string())
            .unwrap();

        // Start consuming
        let result = plugin.start_consuming(consumer).await;
        assert!(result.is_ok());

        // Check that shutdown sender is set
        assert!(plugin.shutdown_tx.is_some());

        // Stop consuming
        let result = plugin.stop_consuming().await;
        assert!(result.is_ok());

        // Check that shutdown sender is consumed (cleared)
        assert!(plugin.shutdown_tx.is_none());
    }
}
