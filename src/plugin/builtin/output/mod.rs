//! Output Plugin - Data Export and Formatting System
//!
//! The OutputPlugin serves as a fallback output plugin when no external output plugins
//! are available. It follows the PluginType::Output pattern and handles data export
//! from other processing plugins using an event-driven architecture.

pub mod args;
pub mod events;
pub mod formats;
pub mod output;
pub mod traits;

use crate::builtin;
use crate::notifications::api::{AsyncNotificationManager, EventFilter};
use crate::plugin::api::Plugin;
use crate::plugin::api::PluginConfig;
use crate::plugin::builtin::output::events::OutputEventHandler;
use crate::plugin::builtin::output::traits::ExportFormat;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::types::{PluginInfo, PluginType};
use crate::scanner::types::ScanRequires;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Register this builtin plugin for automatic discovery
builtin!(|| crate::plugin::discovery::DiscoveredPlugin {
    info: OutputPlugin::static_plugin_info(),
    factory: Box::new(|| Box::new(OutputPlugin::new())),
});

/// OutputPlugin for handling final data output and formatting
pub struct OutputPlugin {
    initialized: bool,
    output_destination: Option<String>,
    template_path: Option<String>,
    /// Detected output format based on function invocation
    detected_format: Option<ExportFormat>,
    /// Injected notification manager
    notification_manager: Option<Arc<Mutex<AsyncNotificationManager>>>,
}

impl OutputPlugin {
    /// Create a new OutputPlugin instance
    pub fn new() -> Self {
        Self {
            initialized: false,
            output_destination: None,
            template_path: None,
            detected_format: None,
            notification_manager: None,
        }
    }

    /// Get static plugin info without creating instance
    pub fn static_plugin_info() -> PluginInfo {
        PluginInfo {
            name: "output".to_string(),
            version: "1.0.0".to_string(),
            description: "Built-in output plugin for data export and formatting".to_string(),
            author: "repostats built-in".to_string(),
            api_version: crate::core::version::get_api_version(),
            plugin_type: PluginType::Output,
            functions: std::iter::once("output".to_string())
                .chain(ExportFormat::names().map(|fmt| fmt.to_string()))
                .filter(|name| name != "j2")
                .collect(),
            required: ScanRequires::NONE,
            auto_active: true,
        }
    }

    /// Check if output destination is stdout or '-'
    fn is_stdout_output(&self) -> bool {
        match &self.output_destination {
            Some(dest) => dest == "-" || dest.is_empty(),
            None => false, // No destination specified = no output configured yet = no progress suppression
        }
    }
}

impl Default for OutputPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Plugin for OutputPlugin {
    fn plugin_info(&self) -> PluginInfo {
        Self::static_plugin_info()
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Output
    }

    fn advertised_functions(&self) -> Vec<String> {
        self.plugin_info().functions
    }

    fn requirements(&self) -> ScanRequires {
        // Only suppress progress if actually outputting to stdout
        if self.is_stdout_output() {
            ScanRequires::SUPPRESS_PROGRESS
        } else {
            ScanRequires::NONE
        }
    }

    fn is_compatible(&self, system_api_version: u32) -> bool {
        // Builtin plugins require system API version to be at least the current version
        system_api_version >= crate::core::version::get_api_version()
    }

    fn set_notification_manager(&mut self, manager: Arc<Mutex<AsyncNotificationManager>>) {
        self.notification_manager = Some(manager);
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        if self.initialized {
            return Err(PluginError::Generic {
                message: "OutputPlugin already initialized".to_string(),
            });
        }

        self.initialized = true;
        Ok(())
    }

    async fn execute(&mut self) -> PluginResult<()> {
        log::debug!("OutputPlugin starting - creating and spawning OutputEventHandler");

        // Get the injected notification manager (cloning for the event handler)
        let notification_manager =
            self.notification_manager
                .clone()
                .ok_or_else(|| {
                    PluginError::Generic {
                message:
                    "Notification manager not set - ensure set_notification_manager() was called"
                        .to_string(),
            }
                })?;

        // Subscribe to all events to get an EventReceiver
        let receiver = {
            let mut manager_guard = notification_manager.lock().await;
            manager_guard
                .subscribe(
                    format!("{}-all-events", self.plugin_info().name),
                    EventFilter::All,
                    format!("OutputPlugin-{}", self.plugin_info().name),
                )
                .map_err(|e| PluginError::Generic {
                    message: format!("Failed to subscribe to events: {}", e),
                })?
        };

        // Create OutputEventHandler with all required resources
        let handler = OutputEventHandler::new(
            self.plugin_info().name.clone(),
            notification_manager,
            Arc::new(Mutex::new(HashMap::new())), // received_data - Arc needed for worker task
        );

        // Spawn the event handler task
        tokio::spawn(handler.run_event_loop(receiver));

        log::debug!("OutputPlugin spawned OutputEventHandler task and returning");
        Ok(())
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
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
