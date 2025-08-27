//! Dump Plugin - Output queue messages to stdout for debugging
//!
//! This plugin consumes messages from the global queue and outputs them in human-readable
//! format to stdout. It's useful for debugging and monitoring message flow.

use crate::plugin::traits::{ConsumerPlugin, Plugin, PluginFunction, PluginInfo, PluginType};
use crate::plugin::{PluginError, PluginResult};
use crate::queue::{Message, QueueConsumer};
use log::{debug, error, info};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Dump plugin for outputting queue messages to stdout
pub struct DumpPlugin {
    /// Plugin initialization state
    initialized: bool,

    /// Consumer for reading messages from the queue
    consumer: Arc<RwLock<Option<QueueConsumer>>>,

    /// Whether the plugin is actively consuming
    consuming: Arc<RwLock<bool>>,

    /// Plugin settings parsed from CLI arguments
    format_json: bool,
    show_headers: bool,
    max_messages: Option<usize>,
}

impl DumpPlugin {
    /// Create a new dump plugin instance
    pub fn new() -> Self {
        Self {
            initialized: false,
            consumer: Arc::new(RwLock::new(None)),
            consuming: Arc::new(RwLock::new(false)),
            format_json: false,
            show_headers: true,
            max_messages: None,
        }
    }

    /// Format a message for output
    fn format_message(&self, message: &Message) -> String {
        if self.format_json {
            self.format_message_json(message)
        } else {
            self.format_message_text(message)
        }
    }

    /// Format message as JSON
    fn format_message_json(&self, message: &Message) -> String {
        // Simple JSON-like output (not using serde to avoid dependencies)
        format!(
            r#"{{"sequence":{},"producer_id":"{}","message_type":"{}","timestamp":"{}","data":"{}"}}"#,
            message.header.sequence,
            message.header.producer_id,
            message.header.message_type,
            message
                .header
                .timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            message.data.replace('"', r#"\""#) // Escape quotes for JSON
        )
    }

    /// Format message as human-readable text
    fn format_message_text(&self, message: &Message) -> String {
        if self.show_headers {
            format!(
                "[{}] {} from {}: {}",
                message.header.sequence,
                message.header.message_type,
                message.header.producer_id,
                message.data
            )
        } else {
            message.data.clone()
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
        f.debug_struct("DumpPlugin")
            .field("initialized", &self.initialized)
            .field("format_json", &self.format_json)
            .field("show_headers", &self.show_headers)
            .field("max_messages", &self.max_messages)
            .field("consumer", &"<consumer>")
            .field("consuming", &"<consuming>")
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

        // Clear consumer
        let mut consumer = self.consumer.write().await;
        *consumer = None;

        self.initialized = false;
        info!("DumpPlugin: Cleanup completed");
        Ok(())
    }

    async fn parse_plugin_arguments(&mut self, args: &[String]) -> PluginResult<()> {
        debug!("DumpPlugin: Parsing arguments: {:?}", args);

        // Reset to defaults
        self.format_json = false;
        self.show_headers = true;
        self.max_messages = None;

        // Parse arguments
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--json" => {
                    self.format_json = true;
                }
                "--no-headers" => {
                    self.show_headers = false;
                }
                "--max-messages" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::Generic {
                            message: "--max-messages requires a number".to_string(),
                        });
                    }
                    i += 1;
                    match args[i].parse::<usize>() {
                        Ok(count) => self.max_messages = Some(count),
                        Err(_) => {
                            return Err(PluginError::Generic {
                                message: format!("Invalid number for --max-messages: {}", args[i]),
                            });
                        }
                    }
                }
                "--help" | "-h" => {
                    return Err(PluginError::Generic {
                        message: "Dump Plugin Help:\n\n\
                            USAGE: dump [OPTIONS]\n\n\
                            OPTIONS:\n\
                            --json           Output messages in JSON format\n\
                            --no-headers     Don't show message headers (sequence, producer, etc.)\n\
                            --max-messages N Stop after N messages\n\
                            --help, -h       Show this help message\n\n\
                            DESCRIPTION:\n\
                            The dump plugin reads messages from the global queue and outputs them\n\
                            to stdout. This is useful for debugging and monitoring message flow.\n\
                            By default, messages are formatted as human-readable text with headers.".to_string(),
                    });
                }
                arg => {
                    return Err(PluginError::Generic {
                        message: format!("Unknown argument: {}", arg),
                    });
                }
            }
            i += 1;
        }

        debug!(
            "DumpPlugin: Arguments parsed - json={}, headers={}, max_messages={:?}",
            self.format_json, self.show_headers, self.max_messages
        );

        Ok(())
    }
}

#[async_trait::async_trait]
impl ConsumerPlugin for DumpPlugin {
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        debug!("DumpPlugin: Starting message consumption");

        // Store the consumer
        let mut consumer_guard = self.consumer.write().await;
        *consumer_guard = Some(consumer);
        drop(consumer_guard);

        // Set consuming flag
        let mut consuming = self.consuming.write().await;
        *consuming = true;
        drop(consuming);

        // Start the consumption task
        let consumer_clone = Arc::clone(&self.consumer);
        let consuming_clone = Arc::clone(&self.consuming);
        let format_json = self.format_json;
        let show_headers = self.show_headers;
        let max_messages = self.max_messages;

        tokio::spawn(async move {
            let mut message_count = 0;

            info!("DumpPlugin: Message consumption started");

            loop {
                // Check if we should still be consuming
                {
                    let consuming_guard = consuming_clone.read().await;
                    if !*consuming_guard {
                        break;
                    }
                }

                // Try to read a message
                let message = {
                    let consumer_guard = consumer_clone.read().await;
                    if let Some(ref consumer) = *consumer_guard {
                        match consumer.read() {
                            Ok(Some(msg)) => Some(msg),
                            Ok(None) => None, // No messages available
                            Err(e) => {
                                error!("DumpPlugin: Error reading message: {:?}", e);
                                None
                            }
                        }
                    } else {
                        break; // Consumer was removed
                    }
                };

                if let Some(msg) = message {
                    // Format and output the message
                    let formatted = if format_json {
                        format!(
                            r#"{{"sequence":{},"producer_id":"{}","message_type":"{}","timestamp":"{}","data":"{}"}}"#,
                            msg.header.sequence,
                            msg.header.producer_id,
                            msg.header.message_type,
                            msg.header
                                .timestamp
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            msg.data.replace('"', r#"\""#) // Escape quotes for JSON
                        )
                    } else if show_headers {
                        format!(
                            "[{}] {} from {}: {}",
                            msg.header.sequence,
                            msg.header.message_type,
                            msg.header.producer_id,
                            msg.data
                        )
                    } else {
                        msg.data.clone()
                    };

                    println!("{}", formatted);
                    message_count += 1;

                    // Check if we've reached the maximum number of messages
                    if let Some(max) = max_messages {
                        if message_count >= max {
                            info!(
                                "DumpPlugin: Reached maximum message count ({}), stopping",
                                max
                            );
                            break;
                        }
                    }
                } else {
                    // No messages available, brief sleep to avoid busy waiting
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }

            info!("DumpPlugin: Message consumption stopped");
        });

        info!("DumpPlugin: Consumer task started");
        Ok(())
    }

    async fn stop_consuming(&mut self) -> PluginResult<()> {
        debug!("DumpPlugin: Stopping message consumption");

        // Set consuming flag to false
        let mut consuming = self.consuming.write().await;
        *consuming = false;

        info!("DumpPlugin: Message consumption stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{Message, QueueManager};

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
        assert!(!plugin.format_json);
        assert!(plugin.show_headers);
        assert!(plugin.max_messages.is_none());
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
        let result = plugin.parse_plugin_arguments(&["--json".to_string()]).await;
        assert!(result.is_ok());
        assert!(plugin.format_json);

        // Test no headers flag
        let result = plugin
            .parse_plugin_arguments(&["--no-headers".to_string()])
            .await;
        assert!(result.is_ok());
        assert!(!plugin.show_headers);

        // Test max messages
        let result = plugin
            .parse_plugin_arguments(&["--max-messages".to_string(), "100".to_string()])
            .await;
        assert!(result.is_ok());
        assert_eq!(plugin.max_messages, Some(100));

        // Test combined flags
        let result = plugin
            .parse_plugin_arguments(&[
                "--json".to_string(),
                "--no-headers".to_string(),
                "--max-messages".to_string(),
                "50".to_string(),
            ])
            .await;
        assert!(result.is_ok());
        assert!(plugin.format_json);
        assert!(!plugin.show_headers);
        assert_eq!(plugin.max_messages, Some(50));
    }

    #[tokio::test]
    async fn test_dump_plugin_argument_parsing_errors() {
        let mut plugin = DumpPlugin::new();

        // Test missing argument for max-messages
        let result = plugin
            .parse_plugin_arguments(&["--max-messages".to_string()])
            .await;
        assert!(result.is_err());

        // Test invalid number for max-messages
        let result = plugin
            .parse_plugin_arguments(&["--max-messages".to_string(), "invalid".to_string()])
            .await;
        assert!(result.is_err());

        // Test unknown argument
        let result = plugin
            .parse_plugin_arguments(&["--unknown".to_string()])
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dump_plugin_help() {
        let mut plugin = DumpPlugin::new();

        // Test help flag
        let result = plugin.parse_plugin_arguments(&["--help".to_string()]).await;
        assert!(result.is_err());

        if let Err(PluginError::Generic { message }) = result {
            assert!(message.contains("Dump Plugin Help"));
            assert!(message.contains("--json"));
            assert!(message.contains("--no-headers"));
            assert!(message.contains("--max-messages"));
        } else {
            panic!("Expected Generic error with help message");
        }

        // Test short help flag
        let result = plugin.parse_plugin_arguments(&["-h".to_string()]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dump_plugin_message_formatting() {
        let plugin = DumpPlugin::new();

        let message = Message::new(
            "test-producer".to_string(),
            "test-event".to_string(),
            "test data".to_string(),
        );

        // Test text format with headers
        let formatted = plugin.format_message(&message);
        assert!(formatted.contains("test-event"));
        assert!(formatted.contains("test-producer"));
        assert!(formatted.contains("test data"));

        // Test JSON format
        let mut json_plugin = DumpPlugin::new();
        json_plugin.format_json = true;
        let formatted = json_plugin.format_message(&message);
        assert!(formatted.contains("\"producer_id\":\"test-producer\""));
        assert!(formatted.contains("\"message_type\":\"test-event\""));
        assert!(formatted.contains("\"data\":\"test data\""));

        // Test no headers format
        let mut no_headers_plugin = DumpPlugin::new();
        no_headers_plugin.show_headers = false;
        let formatted = no_headers_plugin.format_message(&message);
        assert_eq!(formatted, "test data");
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

        // Check that consuming flag is set
        let consuming = plugin.consuming.read().await;
        assert!(*consuming);
        drop(consuming);

        // Stop consuming
        let result = plugin.stop_consuming().await;
        assert!(result.is_ok());

        // Check that consuming flag is cleared
        let consuming = plugin.consuming.read().await;
        assert!(!*consuming);
    }
}
