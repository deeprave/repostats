//! Dump Plugin - Output queue messages to stdout for debugging
//!
//! This plugin consumes messages from the global queue and outputs them in various formats
//! to stdout.
//! It's useful for debugging and monitoring message flow.

use crate::plugin::args::{
    create_format_args, determine_format, OutputFormat, PluginArgParser, PluginConfig,
};
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::traits::{ConsumerPlugin, Plugin};
use crate::plugin::types::{PluginFunction, PluginInfo, PluginType};
use crate::queue::api::{QueueConsumer, QueueError};
use crate::queue::typed::{TypedMessage, TypedQueueConsumer};
use crate::scanner::api::ScanMessage;
use crate::scanner::types::ScanRequires;
use clap::Arg;
use log::error;
use serde_json::json;
use std::collections::HashSet;
use tokio::sync::oneshot;

/// Dump plugin for outputting queue messages to stdout
pub struct DumpPlugin {
    /// Plugin initialization state
    initialized: bool,

    /// Output format setting
    output_format: OutputFormat,

    /// Whether to show message headers
    show_headers: bool,

    /// Whether to request file content from scanner (enables checkout mode)
    request_file_content: bool,

    /// Whether to send keep-alive events to prevent timeout
    send_keepalive: bool,

    /// Keep-alive interval in seconds (default: 10)
    keepalive_interval_secs: u64,

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
            request_file_content: false,
            send_keepalive: false,       // Default disabled
            keepalive_interval_secs: 10, // Default 10 seconds
            shutdown_tx: None,
        }
    }

    /// Format a message header in standard format
    fn format_header(sequence: u64, message_type: &str, producer_id: &str, data: &str) -> String {
        format!(
            "[{}] {} from {}: {}",
            sequence, message_type, producer_id, data
        )
    }

    /// Format typed message directly without recreating raw message
    fn format_typed_message_direct(
        typed_msg: &TypedMessage<ScanMessage>,
        output_format: OutputFormat,
        show_headers: bool,
    ) -> String {
        match output_format {
            OutputFormat::Json => Self::format_json_typed(typed_msg),
            OutputFormat::Compact => Self::format_compact_typed(typed_msg),
            OutputFormat::Text => Self::format_text_typed(typed_msg, show_headers),
        }
    }

    /// Format typed message in JSON format
    fn format_json_typed(typed_msg: &TypedMessage<ScanMessage>) -> String {
        json!({
            "sequence": typed_msg.header.sequence,
            "producer_id": typed_msg.header.producer_id,
            "message_type": typed_msg.header.message_type,
            "timestamp": typed_msg.header
                .timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "scan_message": typed_msg.content
        })
        .to_string()
    }

    /// Format typed message in compact format
    fn format_compact_typed(typed_msg: &TypedMessage<ScanMessage>) -> String {
        if typed_msg.header.message_type.starts_with("scan_started") {
            if let ScanMessage::ScanStarted {
                repository_data, ..
            } = &typed_msg.content
            {
                format!(
                    "{}:{}:repository[{}]:{:?}",
                    typed_msg.header.sequence,
                    typed_msg.header.producer_id,
                    repository_data.path,
                    repository_data.git_ref
                )
            } else {
                format!(
                    "{}:{}:{}:{}",
                    typed_msg.header.sequence,
                    typed_msg.header.producer_id,
                    typed_msg.header.message_type,
                    serde_json::to_string(&typed_msg.content).unwrap_or_default()
                )
            }
        } else {
            format!(
                "{}:{}:{}:{}",
                typed_msg.header.sequence,
                typed_msg.header.producer_id,
                typed_msg.header.message_type,
                serde_json::to_string(&typed_msg.content).unwrap_or_default()
            )
        }
    }

    /// Format typed message in text format
    fn format_text_typed(typed_msg: &TypedMessage<ScanMessage>, show_headers: bool) -> String {
        if typed_msg.header.message_type.starts_with("scan_started") {
            Self::format_repository_data_text_typed(typed_msg, show_headers)
        } else {
            Self::format_regular_message_text_typed(typed_msg, show_headers)
        }
    }

    /// Format repository data message from typed message in text format
    fn format_repository_data_text_typed(
        typed_msg: &TypedMessage<ScanMessage>,
        show_headers: bool,
    ) -> String {
        if let ScanMessage::ScanStarted {
            repository_data, ..
        } = &typed_msg.content
        {
            if show_headers {
                format!(
                    "[{}] Repository Scan Metadata:\n  Path: {}\n  Branch: {:?}\n  Filters: {} files, {} authors, {:?} commits max\n  Date Range: {:?}",
                    typed_msg.header.sequence,
                    repository_data.path,
                    repository_data.git_ref.as_deref().unwrap_or("default"),
                    repository_data.file_paths.as_deref().unwrap_or("all"),
                    repository_data.authors.as_deref().unwrap_or("all"),
                    repository_data.max_commits,
                    repository_data.date_range.as_deref().unwrap_or("all time")
                )
            } else {
                format!(
                    "Repository: {} (branch: {:?}, filters: active)",
                    repository_data.path, repository_data.git_ref
                )
            }
        } else {
            // Fallback if message type doesn't match content
            Self::format_regular_message_text_typed(typed_msg, show_headers)
        }
    }

    /// Format regular (non-repository) typed message in text format
    fn format_regular_message_text_typed(
        typed_msg: &TypedMessage<ScanMessage>,
        show_headers: bool,
    ) -> String {
        if show_headers {
            Self::format_header(
                typed_msg.header.sequence,
                &typed_msg.header.message_type,
                &typed_msg.header.producer_id,
                &serde_json::to_string(&typed_msg.content).unwrap_or_default(),
            )
        } else {
            serde_json::to_string(&typed_msg.content).unwrap_or_default()
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
            .field("output_format", &self.output_format)
            .field("show_headers", &self.show_headers)
            .field("request_file_content", &self.request_file_content)
            .field("send_keepalive", &self.send_keepalive)
            .field("keepalive_interval_secs", &self.keepalive_interval_secs)
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
            description: "Dump repository data for debugging purposes".to_string(),
            author: "RepoStats".to_string(),
            api_version: 20250101,
            plugin_type: self.plugin_type(),
            functions: self.advertised_functions(),
            required: self.requirements(),
            auto_active: false, // Dump is activated explicitly
        }
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Processing
    }

    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![PluginFunction {
            name: "dump".to_string(),
            description: "Start dumping messages to stdout".to_string(),
            aliases: vec!["start".to_string(), "run".to_string()],
        }]
    }

    fn requirements(&self) -> ScanRequires {
        // Dump plugin needs comprehensive data for debugging purposes
        // Including repository info, history, and file changes for complete debugging visibility
        let mut reqs =
            ScanRequires::REPOSITORY_INFO | ScanRequires::HISTORY | ScanRequires::FILE_CHANGES;

        // If --checkout flag was used, also request file content for testing historical reconstruction
        if self.request_file_content {
            reqs |= ScanRequires::FILE_CONTENT;
        }

        reqs
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        self.initialized = true;
        Ok(())
    }

    async fn execute(&mut self, _args: &[String]) -> PluginResult<()> {
        if !self.initialized {
            return Err(PluginError::ExecutionError {
                plugin_name: "dump".to_string(),
                operation: "execute".to_string(),
                cause: "Plugin not initialized".to_string(),
            });
        }

        // For now, just log that execute was called
        // The actual message consumption happens via ConsumerPlugin trait
        Ok(())
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        // Stop consuming first
        let _ = self.stop_consuming().await;

        self.initialized = false;
        Ok(())
    }

    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()> {
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
        )
        .arg(
            Arg::new("checkout")
                .long("checkout")
                .action(clap::ArgAction::SetTrue)
                .help("Request file content from scanner (enables historical file reconstruction testing)"),
        )
        .arg(
            Arg::new("keepalive")
                .long("keepalive")
                .action(clap::ArgAction::SetTrue)
                .help("Send keep-alive events during processing to prevent plugin timeout"),
        )
        .arg(
            Arg::new("keepalive-interval")
                .long("keepalive-interval")
                .value_name("SECONDS")
                .help("Keep-alive interval in seconds (default: 10, minimum: 1)")
                .value_parser(clap::value_parser!(u64))
                .default_value("10"),
        );

        // Parse arguments using clap
        let matches = parser.parse(args)?;

        // Determine format from arguments and configuration
        self.output_format = determine_format(&matches, config);
        self.show_headers = !matches.get_flag("no-headers");
        self.request_file_content = matches.get_flag("checkout");
        self.send_keepalive = matches.get_flag("keepalive");

        // Parse keep-alive interval with validation
        const MAX_KEEPALIVE_INTERVAL_SECS: u64 = 3600; // 1 hour upper limit
        if let Some(interval_secs) = matches.get_one::<u64>("keepalive-interval") {
            if *interval_secs == 0 {
                return Err(PluginError::Generic {
                    message: "Keep-alive interval must be at least 1 second".to_string(),
                });
            }
            if *interval_secs > MAX_KEEPALIVE_INTERVAL_SECS {
                return Err(PluginError::Generic {
                    message: format!(
                        "Keep-alive interval must not exceed {} seconds (got {})",
                        MAX_KEEPALIVE_INTERVAL_SECS, interval_secs
                    ),
                });
            }
            self.keepalive_interval_secs = *interval_secs;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ConsumerPlugin for DumpPlugin {
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        // Create shutdown channel for graceful shutdown
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        // Create typed consumer for ScanMessage
        let typed_consumer = TypedQueueConsumer::<ScanMessage>::new(consumer);

        // Capture plugin settings for the task
        let output_format = self.output_format;
        let show_headers = self.show_headers;
        let send_keepalive = self.send_keepalive;
        let keepalive_interval_secs = self.keepalive_interval_secs;
        let plugin_name = self.plugin_info().name;

        // Spawn the consumer task that owns the consumer directly
        tokio::spawn(async move {
            log::trace!(
                "DumpPlugin: Starting consumer task for plugin '{}'",
                plugin_name
            );
            let mut message_count = 0;
            let mut active_scanners = HashSet::new();
            let mut completed_scanners = HashSet::new();

            // Circuit breaker for keep-alive failures
            let mut consecutive_keepalive_failures = 0;
            const MAX_CONSECUTIVE_KEEPALIVE_FAILURES: usize = 3;

            // Setup keep-alive timer if enabled
            let mut keepalive_interval = if send_keepalive {
                log::trace!(
                    "DumpPlugin: Keep-alive enabled with interval {}s",
                    keepalive_interval_secs
                );
                Some(tokio::time::interval(tokio::time::Duration::from_secs(
                    keepalive_interval_secs,
                )))
            } else {
                log::trace!("DumpPlugin: Keep-alive disabled");
                None
            };

            log::trace!("DumpPlugin: Entering main message processing loop");
            loop {
                tokio::select! {
                    // Check for shutdown signal
                    _ = &mut shutdown_rx => {
                        log::debug!("DumpPlugin: Received shutdown signal, stopping...");

                        // Publish plugin completion event for shutdown scenario
                        if let Err(e) = crate::plugin::events::publish_plugin_completion_event(
                            &plugin_name,
                            "Plugin shutdown requested - stopping gracefully"
                        ).await {
                            log::error!("Failed to publish plugin completion event on shutdown: {}", e);
                        }
                        break;
                    }

                    // Send keep-alive events if enabled
                    _ = async {
                        if let Some(ref mut interval) = keepalive_interval {
                            interval.tick().await;
                        } else {
                            // Keep this branch alive but never complete if keep-alive is disabled
                            std::future::pending::<()>().await;
                        }
                    } => {
                        // Retry keep-alive event publishing up to 3 times with 1s delay between attempts
                        let mut retries = 0;
                        let max_retries = 3;
                        let mut last_err = None;
                        while retries < max_retries {
                            match crate::plugin::events::publish_plugin_keepalive_event(
                                &plugin_name,
                                &format!("Processed {} messages", message_count)
                            ).await {
                                Ok(_) => {
                                    log::trace!("DumpPlugin: Sent keep-alive event (processed {} messages)", message_count);
                                    last_err = None;
                                    break;
                                }
                                Err(e) => {
                                    last_err = Some(e);
                                    retries += 1;
                                    if retries < max_retries {
                                        log::warn!("Failed to publish keep-alive event (attempt {}/{}), retrying: {}", retries, max_retries, last_err.as_ref().unwrap());
                                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                    }
                                }
                            }
                        }
                        if let Some(e) = last_err {
                            log::error!("Failed to publish keep-alive event after {} attempts: {}", max_retries, e);

                            // Circuit breaker: disable keep-alive after too many consecutive failures
                            consecutive_keepalive_failures += 1;
                            if consecutive_keepalive_failures >= MAX_CONSECUTIVE_KEEPALIVE_FAILURES {
                                log::error!(
                                    "Circuit breaker triggered: Disabling keep-alive after {} consecutive failed retry cycles",
                                    consecutive_keepalive_failures
                                );
                                keepalive_interval = None; // Disable keep-alive to prevent further failures
                            }
                        } else {
                            // Reset counter on successful keep-alive
                            consecutive_keepalive_failures = 0;
                        }
                    }

                    // Try to read a typed message (with timeout to allow shutdown checks)
                    result = tokio::time::timeout(
                        tokio::time::Duration::from_millis(1000), // Increased from 100ms to reduce busy waiting
                        async { typed_consumer.read_with_header() }
                    ) => {
                        match result {
                            Ok(Ok(Some(typed_msg))) => {
                                log::trace!("DumpPlugin: Received message #{} of type {:?}", message_count + 1, std::mem::discriminant(&typed_msg.content));
                                let formatted = DumpPlugin::format_typed_message_direct(&typed_msg, output_format, show_headers);
                                println!("{}", formatted);
                                message_count += 1;

                                // Track scanner lifecycle using typed content directly
                                match &typed_msg.content {
                                    ScanMessage::ScanStarted { scanner_id, .. } => {
                                        active_scanners.insert(scanner_id.clone());
                                        log::trace!("DumpPlugin: Started tracking scanner: {} (active: {:?})", scanner_id, active_scanners);
                                    }
                                    ScanMessage::ScanCompleted { scanner_id, .. } => {
                                            completed_scanners.insert(scanner_id.clone());
                                            log::trace!("DumpPlugin: Scanner completed: {} (active: {:?}, completed: {:?})", scanner_id, active_scanners, completed_scanners);

                                            // Check if all active scanners have completed
                                            let all_completed = !active_scanners.is_empty() &&
                                               active_scanners.iter().all(|id| completed_scanners.contains(id));
                                            log::trace!("DumpPlugin: Completion check - active_empty: {}, all_completed: {}", active_scanners.is_empty(), all_completed);

                                            if all_completed {
                                                log::debug!("DumpPlugin: All scanners completed, finishing...");

                                                // Publish plugin completion event
                                                log::trace!("DumpPlugin: Publishing plugin completion event");
                                                if let Err(e) = crate::plugin::events::publish_plugin_completion_event(
                                                    &plugin_name,
                                                    "All scanners completed - plugin processing finished"
                                                ).await {
                                                    log::error!("Failed to publish plugin completion event: {}", e);
                                                } else {
                                                    log::trace!("DumpPlugin: Successfully published plugin completion event");
                                                }
                                                break;
                                            }
                                    }
                                    ScanMessage::ScanError { scanner_id, .. } => {
                                        completed_scanners.insert(scanner_id.clone());
                                        log::trace!("DumpPlugin: Scanner failed: {} (active: {:?}, completed: {:?})", scanner_id, active_scanners, completed_scanners);

                                        // Check if all active scanners have completed (including errors)
                                        let all_completed = !active_scanners.is_empty() &&
                                           active_scanners.iter().all(|id| completed_scanners.contains(id));
                                        log::trace!("DumpPlugin: Error completion check - active_empty: {}, all_completed: {}", active_scanners.is_empty(), all_completed);

                                        if all_completed {
                                            log::debug!("DumpPlugin: All scanners completed (some with errors), finishing...");

                                            // Publish plugin completion event
                                            log::trace!("DumpPlugin: Publishing plugin completion event after errors");
                                            if let Err(e) = crate::plugin::events::publish_plugin_completion_event(
                                                &plugin_name,
                                                "All scanners completed - plugin processing finished"
                                            ).await {
                                                log::error!("Failed to publish plugin completion event: {}", e);
                                            } else {
                                                log::trace!("DumpPlugin: Successfully published plugin completion event after errors");
                                            }
                                            break;
                                        }
                                    }
                                    _ => {
                                        log::trace!("DumpPlugin: Received non-lifecycle message, continuing");
                                    }
                                }
                            }
                            Ok(Ok(None)) => {
                                // No messages available, continue the loop
                                // (timeout will naturally provide a small delay)
                            }
                            Ok(Err(e)) => {
                                match &e {
                                    QueueError::DeserializationError { message } => {
                                        error!("DumpPlugin: Message deserialization failed: {}", message);
                                    }
                                    _ => {
                                        error!("DumpPlugin: Error reading typed message: {:?}", e);
                                    }
                                }
                                // Continue processing despite errors
                            }
                            Err(_) => {
                                // Timeout occurred, continue loop to check for shutdown
                            }
                        }
                    }
                }
            }

            log::trace!("DumpPlugin: Exiting message processing loop. Final state - active_scanners: {:?}, completed_scanners: {:?}", active_scanners, completed_scanners);
            log::info!("DumpPlugin: Processed {} messages total", message_count);
        });

        // Store the shutdown sender for lifecycle management
        self.shutdown_tx = Some(shutdown_tx);

        Ok(())
    }

    async fn stop_consuming(&mut self) -> PluginResult<()> {
        // Send shutdown signal to the task
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            if let Err(_) = shutdown_tx.send(()) {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::api::QueueManager;

    #[tokio::test]
    async fn test_dump_plugin_creation() {
        let plugin = DumpPlugin::new();

        assert!(!plugin.initialized);
        assert_eq!(plugin.plugin_info().name, "dump");
        assert_eq!(plugin.plugin_info().version, "1.0.0");
        assert_eq!(plugin.plugin_type(), PluginType::Processing);
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

        // Test checkout flag
        let result = plugin
            .parse_plugin_arguments(&["--checkout".to_string()], &config)
            .await;
        assert!(result.is_ok());
        assert!(plugin.request_file_content);

        // Test combined flags
        let result = plugin
            .parse_plugin_arguments(
                &[
                    "--json".to_string(),
                    "--no-headers".to_string(),
                    "--checkout".to_string(),
                ],
                &config,
            )
            .await;
        assert!(result.is_ok());
        assert_eq!(plugin.output_format, OutputFormat::Json);
        assert!(!plugin.show_headers);
        assert!(plugin.request_file_content);
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

    #[tokio::test]
    async fn test_dump_plugin_typed_consumer_integration() {
        use crate::queue::api::Message;
        use crate::scanner::api::ScanMessage;
        use crate::scanner::types::{RepositoryData, ScanStats};
        use std::time::{Duration, SystemTime};

        let mut plugin = DumpPlugin::new();
        let queue_manager = QueueManager::create().await;

        // Initialize and configure plugin
        plugin.initialize().await.unwrap();
        let config = PluginConfig::default();
        plugin
            .parse_plugin_arguments(&["--json".to_string()], &config)
            .await
            .unwrap();

        // Create publisher and consumer
        let publisher = queue_manager
            .create_publisher("scanner".to_string())
            .unwrap();
        let consumer = queue_manager
            .create_consumer("dump-test".to_string())
            .unwrap();

        // Publish a ScanStarted message
        let scan_started = ScanMessage::ScanStarted {
            scanner_id: "integration-test-scanner".to_string(),
            timestamp: SystemTime::now(),
            repository_data: RepositoryData {
                path: "/test/repo".to_string(),
                url: None,
                name: Some("test-repo".to_string()),
                description: Some("Test repository".to_string()),
                default_branch: Some("main".to_string()),
                is_bare: false,
                is_shallow: false,
                work_dir: Some("/test/repo".to_string()),
                git_dir: "/test/repo/.git".to_string(),
                git_ref: Some("main".to_string()),
                date_range: None,
                file_paths: None,
                authors: None,
                max_commits: Some(10),
            },
        };

        let json_data = serde_json::to_string(&scan_started).unwrap();
        let message = Message::new("scanner".to_string(), "scan_started".to_string(), json_data);
        publisher.publish(message).unwrap();

        // Start consuming (spawns background task)
        plugin.start_consuming(consumer).await.unwrap();

        // Give the background task time to process the ScanStarted message
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Publish a ScanCompleted message to trigger shutdown
        let scan_completed = ScanMessage::ScanCompleted {
            scanner_id: "integration-test-scanner".to_string(),
            timestamp: SystemTime::now(),
            stats: ScanStats {
                total_commits: 5,
                total_files_changed: 10,
                total_insertions: 100,
                total_deletions: 50,
                scan_duration: Duration::from_millis(1000),
            },
        };

        let json_data = serde_json::to_string(&scan_completed).unwrap();
        let message = Message::new(
            "scanner".to_string(),
            "scan_completed".to_string(),
            json_data,
        );
        publisher.publish(message).unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Plugin should have automatically stopped consuming due to all scanners completed
        // Verify it can be cleanly shut down
        let result = plugin.stop_consuming().await;
        assert!(result.is_ok());

        // Verify shutdown state
        assert!(plugin.shutdown_tx.is_none());
    }

    #[tokio::test]
    async fn test_dump_plugin_typed_error_handling() {
        use crate::queue::api::Message;
        use std::time::Duration;

        let mut plugin = DumpPlugin::new();
        let queue_manager = QueueManager::create().await;

        // Initialize plugin
        plugin.initialize().await.unwrap();
        let config = PluginConfig::default();
        plugin
            .parse_plugin_arguments(&["--json".to_string()], &config)
            .await
            .unwrap();

        // Create publisher and consumer
        let publisher = queue_manager
            .create_publisher("scanner".to_string())
            .unwrap();
        let consumer = queue_manager
            .create_consumer("error-test".to_string())
            .unwrap();

        // Publish an invalid JSON message that will trigger deserialization error
        let message = Message::new(
            "scanner".to_string(),
            "invalid_message".to_string(),
            "invalid json content".to_string(),
        );
        publisher.publish(message).unwrap();

        // Start consuming
        plugin.start_consuming(consumer).await.unwrap();

        // Give time for error processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Plugin should continue running despite the error
        assert!(plugin.shutdown_tx.is_some());

        // Clean shutdown
        plugin.stop_consuming().await.unwrap();
        assert!(plugin.shutdown_tx.is_none());
    }

    #[tokio::test]
    async fn test_dump_plugin_requirements_with_checkout() {
        // Test without checkout flag
        let plugin = DumpPlugin::new();
        let reqs = plugin.requirements();
        assert!(reqs.requires_repository_info());
        assert!(reqs.requires_history());
        assert!(reqs.requires_file_changes());
        assert!(!reqs.requires_file_content()); // Should not be included without --checkout

        // Test with checkout flag
        let mut plugin_with_checkout = DumpPlugin::new();
        let config = PluginConfig::default();
        let result = plugin_with_checkout
            .parse_plugin_arguments(&["--checkout".to_string()], &config)
            .await;
        assert!(result.is_ok());

        let reqs_with_checkout = plugin_with_checkout.requirements();
        assert!(reqs_with_checkout.requires_repository_info());
        assert!(reqs_with_checkout.requires_history());
        assert!(reqs_with_checkout.requires_file_changes());
        assert!(reqs_with_checkout.requires_file_content()); // Should be included with --checkout
    }
}
