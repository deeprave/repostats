//! Dump Plugin - orchestrator module
//! Split into submodules: args (CLI parsing), format (output formatting), consumer (message loop)

mod args;
mod consumer;
mod format;

use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::args::{OutputFormat, PluginConfig};
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::error_handling::log_plugin_error_with_context;
use crate::plugin::traits::{ConsumerPlugin, Plugin};
use crate::plugin::types::{PluginFunction, PluginInfo, PluginType};
use crate::queue::api::QueueConsumer;
use crate::scanner::types::ScanRequires;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

/// Public dump plugin structure
pub struct DumpPlugin {
    initialized: bool,
    output_format: OutputFormat,
    show_headers: bool,
    request_file_content: bool,
    request_file_info: bool,
    send_keepalive: bool,
    keepalive_interval_secs: u64,
    shutdown_tx: Option<oneshot::Sender<()>>,
    consumer_task: Option<tokio::task::JoinHandle<()>>,
    use_colors: bool,
    output_file: Option<PathBuf>,
    /// Injected notification manager
    notification_manager: Option<Arc<Mutex<AsyncNotificationManager>>>,
}

impl DumpPlugin {
    pub fn new() -> Self {
        Self {
            initialized: false,
            output_format: OutputFormat::Text,
            show_headers: true,
            request_file_content: false,
            request_file_info: false,
            send_keepalive: true,
            keepalive_interval_secs: 10,
            shutdown_tx: None,
            consumer_task: None,
            use_colors: false,
            output_file: None,
            notification_manager: None,
        }
    }

    // Re-export formatting helpers used in tests (delegate to format module)
    #[allow(dead_code)]
    pub(crate) fn format_typed_message_direct(
        typed_msg: &crate::queue::typed::TypedMessage<crate::scanner::api::ScanMessage>,
        output_format: OutputFormat,
        show_headers: bool,
        use_colors: bool,
    ) -> String {
        format::format_typed_message_direct(typed_msg, output_format, show_headers, use_colors)
    }

    #[allow(dead_code)]
    pub(crate) fn format_typed_message_direct_with_color(
        typed_msg: &crate::queue::typed::TypedMessage<crate::scanner::api::ScanMessage>,
        output_format: OutputFormat,
        show_headers: bool,
        use_colors: bool,
    ) -> String {
        format::format_typed_message_direct_with_color(
            typed_msg,
            output_format,
            show_headers,
            use_colors,
        )
    }

    // Accessors for tests
    #[allow(dead_code)]
    pub fn test_output_format(&self) -> OutputFormat {
        self.output_format
    }
    #[allow(dead_code)]
    pub fn test_use_colors(&self) -> bool {
        self.use_colors
    }
    #[allow(dead_code)]
    pub fn test_output_file(&self) -> Option<&std::path::Path> {
        self.output_file.as_deref()
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
            .field("request_file_info", &self.request_file_info)
            .field("send_keepalive", &self.send_keepalive)
            .field("keepalive_interval_secs", &self.keepalive_interval_secs)
            .field("shutdown_tx", &self.shutdown_tx.is_some())
            .field("use_colors", &self.use_colors)
            .finish()
    }
}

#[async_trait::async_trait]
impl Plugin for DumpPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: "dump".into(),
            version: "1.0.0".into(),
            description: "Dump repository data for debugging purposes".into(),
            author: "RepoStats".into(),
            api_version: crate::core::version::get_api_version(),
            plugin_type: self.plugin_type(),
            functions: self.advertised_functions(),
            required: self.requirements(),
            auto_active: false,
        }
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Processing
    }

    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![PluginFunction {
            name: "dump".into(),
            description: "Start dumping messages to stdout".into(),
            aliases: vec!["start".into(), "run".into()],
        }]
    }

    fn requirements(&self) -> ScanRequires {
        let mut reqs =
            ScanRequires::REPOSITORY_INFO | ScanRequires::HISTORY | ScanRequires::COMMITS;
        if self.output_file.is_none() {
            reqs |= ScanRequires::SUPPRESS_PROGRESS;
        }
        if self.request_file_info {
            reqs |= ScanRequires::FILE_INFO;
        }
        if self.request_file_content {
            reqs |= ScanRequires::FILE_CONTENT;
        }
        reqs
    }

    fn set_notification_manager(&mut self, manager: Arc<Mutex<AsyncNotificationManager>>) {
        self.notification_manager = Some(manager);
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        self.initialized = true;
        Ok(())
    }

    async fn execute(&mut self, _args: &[String]) -> PluginResult<()> {
        if !self.initialized {
            let err = PluginError::ExecutionError {
                plugin_name: "dump".into(),
                operation: "execute".into(),
                cause: "Plugin not initialized".into(),
            };
            log_plugin_error_with_context(&err, "Plugin not initialized");
            return Err(err);
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        let _ = self.stop_consuming().await;
        self.initialized = false;
        Ok(())
    }

    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()> {
        self.args_parse(args, config).await
    }
}

#[async_trait::async_trait]
impl ConsumerPlugin for DumpPlugin {
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        self.start_consumer_loop(consumer).await
    }
    async fn stop_consuming(&mut self) -> PluginResult<()> {
        self.stop_consumer_loop().await
    }
}

// Re-export selected formatting functions for external tests if needed
// pub use format::{format_compact_typed, format_json_typed, format_pretty_text_typed};

#[cfg(test)]
mod tests;
