//! Dump Plugin - orchestrator module
//! Split into submodules: args (CLI parsing), format (output formatting), consumer (message loop)

mod args;
mod consumer;
mod format;

use crate::plugin::traits::{ConsumerPlugin, Plugin};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

use crate::builtin;
use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::api::{PluginError, PluginResult};
use crate::plugin::args::PluginConfig;
use crate::plugin::error_handling::log_plugin_error_with_context;
use crate::plugin::types::{PluginInfo, PluginType};
use crate::queue::api::QueueConsumer;
use crate::scanner::api::ScanRequires;

// Register this builtin plugin for automatic discovery
builtin!(|| crate::plugin::discovery::DiscoveredPlugin {
    info: DumpPlugin::static_plugin_info(),
    factory: Box::new(|| Box::new(DumpPlugin::new())),
});

/// Standard output formats for dump
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
    Compact,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Compact => write!(f, "compact"),
        }
    }
}

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

    /// Get static plugin info without creating instance
    pub fn static_plugin_info() -> PluginInfo {
        PluginInfo {
            name: "dump".to_string(),
            version: "1.0.0".to_string(),
            description: "Dump repository data for debugging purposes".to_string(),
            author: "RepoStats".to_string(),
            api_version: crate::core::version::get_api_version(),
            plugin_type: PluginType::Processing,
            functions: vec!["dump".to_string()],
            required: ScanRequires::HISTORY | ScanRequires::COMMITS | ScanRequires::FILE_CONTENT,
            auto_active: false,
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
        Self::static_plugin_info()
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Processing
    }

    fn advertised_functions(&self) -> Vec<String> {
        vec!["dump".to_string()]
    }

    fn requirements(&self) -> ScanRequires {
        let mut reqs = ScanRequires::HISTORY | ScanRequires::COMMITS;
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

    fn is_compatible(&self, system_api_version: u32) -> bool {
        // Builtin plugins require system API version to be at least the current version
        system_api_version >= crate::core::version::get_api_version()
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
        let _ = self.stop_consumer_loop().await;
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

    // Expose ConsumerPlugin via dyn Plugin
    fn as_consumer_plugin(&mut self) -> Option<&mut dyn ConsumerPlugin> {
        Some(self)
    }
}

#[async_trait::async_trait]
impl ConsumerPlugin for DumpPlugin {
    async fn inject_consumer(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        self.start_consumer_loop(consumer).await
    }
}
