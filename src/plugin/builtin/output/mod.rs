//! Output Plugin - Data Export and Formatting System
//!
//! The OutputPlugin serves as a fallback output plugin when no external output plugins
//! are available. It follows the PluginType::Output pattern and handles data export
//! from other processing plugins using an event-driven architecture.

pub mod args;
pub mod formats;
pub mod output;
pub mod traits;

#[cfg(test)]
pub mod tests;

use crate::builtin;
use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::builtin::output::traits::ExportFormat;
use crate::plugin::data_export::PluginDataExport;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::traits::Plugin;
use crate::plugin::types::{PluginInfo, PluginType};
use crate::scanner::types::ScanRequires;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Repository context information from ScanStarted events
#[derive(Clone, Debug)]
pub struct RepositoryContext {
    pub repository_path: String,
    pub git_ref: Option<String>,
    pub scan_id: String,
}

/// OutputPlugin for handling final data output and formatting
pub struct OutputPlugin {
    initialized: bool,
    output_destination: Option<String>,
    template_path: Option<String>,
    /// Detected output format based on function invocation
    detected_format: Option<crate::plugin::builtin::output::traits::ExportFormat>,
    /// Scan ID of data currently being processed
    processing_scan_id: Option<String>,
    /// Repository contexts indexed by scan_id
    repository_contexts: HashMap<String, RepositoryContext>,
    /// Received data exports indexed by (plugin_id, scan_id)
    received_data: HashMap<(String, String), Arc<PluginDataExport>>,
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
            processing_scan_id: None,
            repository_contexts: HashMap::new(),
            received_data: HashMap::new(),
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

    /// Send a keep-alive signal to indicate plugin is still active during data processing
    pub async fn send_keepalive_signal(
        &self,
        status_message: &str,
    ) -> crate::plugin::error::PluginResult<()> {
        if !self.initialized {
            return Err(crate::plugin::error::PluginError::Generic {
                message: "OutputPlugin not initialized".to_string(),
            });
        }

        // Skip keep-alive if not processing any scan data yet
        if self.processing_scan_id.is_none() {
            return Ok(());
        }

        let manager = self.notification_manager.as_ref().ok_or_else(|| {
            crate::plugin::error::PluginError::Generic {
                message: "Notification manager not set".to_string(),
            }
        })?;

        use crate::notifications::api::PluginEvent;
        use crate::notifications::event::{Event, PluginEventType};

        let event = Event::Plugin(PluginEvent::with_message(
            PluginEventType::KeepAlive,
            self.plugin_info().name.clone(),
            self.processing_scan_id
                .as_deref()
                .unwrap_or("no-active-scan")
                .to_string(),
            status_message.to_string(),
        ));

        let mut notification_manager = manager.lock().await;
        notification_manager.publish(event).await.map_err(|e| {
            crate::plugin::error::PluginError::Generic {
                message: format!("Failed to publish keep-alive event: {}", e),
            }
        })?;

        Ok(())
    }

    /// Subscribe to plugin events
    pub async fn subscribe_to_plugin_events(
        &self,
    ) -> crate::plugin::error::PluginResult<crate::notifications::api::EventReceiver> {
        if !self.initialized {
            return Err(PluginError::Generic {
                message: "OutputPlugin not initialized".to_string(),
            });
        }

        let manager = self
            .notification_manager
            .as_ref()
            .ok_or_else(|| PluginError::Generic {
                message: "Notification manager not set".to_string(),
            })?;

        let mut notification_manager = manager.lock().await;
        let receiver = notification_manager
            .subscribe(
                format!("{}-plugin-events", self.plugin_info().name),
                crate::notifications::api::EventFilter::PluginOnly,
                format!("OutputPlugin-{}", self.plugin_info().name),
            )
            .map_err(|e| PluginError::Generic {
                message: format!("Failed to subscribe to plugin events: {}", e),
            })?;

        Ok(receiver)
    }

    /// Subscribe to scan events
    pub async fn subscribe_to_scan_events(
        &self,
    ) -> PluginResult<crate::notifications::api::EventReceiver> {
        if !self.initialized {
            return Err(PluginError::Generic {
                message: "OutputPlugin not initialized".to_string(),
            });
        }

        let manager = self.notification_manager.as_ref().ok_or_else(|| {
            crate::plugin::error::PluginError::Generic {
                message: "Notification manager not set".to_string(),
            }
        })?;

        let mut notification_manager = manager.lock().await;
        let receiver = notification_manager
            .subscribe(
                format!("{}-scan-events", self.plugin_info().name),
                crate::notifications::api::EventFilter::ScanOnly,
                format!("OutputPlugin-{}", self.plugin_info().name),
            )
            .map_err(|e| crate::plugin::error::PluginError::Generic {
                message: format!("Failed to subscribe to scan events: {}", e),
            })?;

        Ok(receiver)
    }

    /// Subscribe to system events
    pub async fn subscribe_to_system_events(
        &self,
    ) -> crate::plugin::error::PluginResult<crate::notifications::api::EventReceiver> {
        if !self.initialized {
            return Err(crate::plugin::error::PluginError::Generic {
                message: "OutputPlugin not initialized".to_string(),
            });
        }

        let manager = self.notification_manager.as_ref().ok_or_else(|| {
            crate::plugin::error::PluginError::Generic {
                message: "Notification manager not set".to_string(),
            }
        })?;

        let mut notification_manager = manager.lock().await;
        let receiver = notification_manager
            .subscribe(
                format!("{}-system-events", self.plugin_info().name),
                crate::notifications::api::EventFilter::SystemOnly,
                format!("OutputPlugin-{}", self.plugin_info().name),
            )
            .map_err(|e| crate::plugin::error::PluginError::Generic {
                message: format!("Failed to subscribe to system events: {}", e),
            })?;

        Ok(receiver)
    }

    /// Handle a plugin event
    pub async fn handle_plugin_event(
        &mut self,
        event: &crate::notifications::api::PluginEvent,
    ) -> crate::plugin::error::PluginResult<()> {
        if !self.initialized {
            return Err(crate::plugin::error::PluginError::Generic {
                message: "OutputPlugin not initialized".to_string(),
            });
        }

        match event.event_type {
            crate::notifications::api::PluginEventType::DataReady => {
                if let Some(data_export) = &event.data_export {
                    log::info!(
                        "OutputPlugin received DataReady from plugin {} for scan {}",
                        event.plugin_id,
                        event.scan_id
                    );
                    self.process_data_ready_event(
                        &event.plugin_id,
                        &event.scan_id,
                        data_export.clone(),
                    )
                    .await?;
                } else {
                    log::warn!(
                        "OutputPlugin received DataReady event from plugin {} without data",
                        event.plugin_id
                    );
                }
            }
            _ => {
                log::debug!("OutputPlugin handling plugin event: {:?}", event);
            }
        }
        Ok(())
    }

    /// Handle a scan event
    pub async fn handle_scan_event(
        &mut self,
        event: &crate::notifications::api::ScanEvent,
    ) -> crate::plugin::error::PluginResult<()> {
        if !self.initialized {
            return Err(crate::plugin::error::PluginError::Generic {
                message: "OutputPlugin not initialized".to_string(),
            });
        }

        match event.event_type {
            crate::notifications::api::ScanEventType::Started => {
                log::debug!(
                    "OutputPlugin received scan started for scan {}",
                    event.scan_id
                );
                // Set the current active scan_id
                self.processing_scan_id = Some(event.scan_id.clone());
                self.store_repository_context(&event.scan_id, event).await?;
            }
            crate::notifications::api::ScanEventType::Error => {
                log::warn!("OutputPlugin received scan error: {:?}", event.message);
                // Handle error scenarios - potentially abort or adjust behavior
            }
            crate::notifications::api::ScanEventType::Completed => {
                log::debug!(
                    "OutputPlugin received scan completion for scan {}",
                    event.scan_id
                );
                // Clear processing scan if this scan has completed
                if self.processing_scan_id.as_ref() == Some(&event.scan_id) {
                    self.processing_scan_id = None;
                }
                // Scan completion doesn't trigger export - DataReady events do
            }
            _ => {
                log::debug!("OutputPlugin handling scan event: {:?}", event);
            }
        }
        Ok(())
    }

    /// Handle a system event
    pub async fn handle_system_event(
        &self,
        event: &crate::notifications::api::SystemEvent,
    ) -> crate::plugin::error::PluginResult<()> {
        if !self.initialized {
            return Err(crate::plugin::error::PluginError::Generic {
                message: "OutputPlugin not initialized".to_string(),
            });
        }

        match event.event_type {
            crate::notifications::api::SystemEventType::Shutdown => {
                log::info!(
                    "OutputPlugin received shutdown signal - preparing for graceful termination"
                );
                // TODO: Ensure any pending exports are completed or saved
            }
            _ => {
                log::debug!("OutputPlugin handling system event: {:?}", event);
            }
        }
        Ok(())
    }

    /// Store repository context from ScanStarted event
    async fn store_repository_context(
        &mut self,
        scan_id: &str,
        event: &crate::notifications::api::ScanEvent,
    ) -> crate::plugin::error::PluginResult<()> {
        // Extract repository information from scan event message
        // For now, use basic extraction - this could be enhanced with proper parsing
        let repository_path = event.message.as_deref().unwrap_or("unknown").to_string();

        let context = RepositoryContext {
            repository_path,
            git_ref: None, // TODO: Extract from scan event when available
            scan_id: scan_id.to_string(),
        };

        self.repository_contexts
            .insert(scan_id.to_string(), context);
        log::debug!("Stored repository context for scan {}", scan_id);
        Ok(())
    }

    /// Process a DataReady event by storing data and triggering immediate export
    async fn process_data_ready_event(
        &mut self,
        plugin_id: &str,
        scan_id: &str,
        data_export: Arc<PluginDataExport>,
    ) -> crate::plugin::error::PluginResult<()> {
        let key = (plugin_id.to_string(), scan_id.to_string());

        // Store the received data
        self.received_data.insert(key.clone(), data_export.clone());

        log::info!(
            "Stored data from plugin {} for scan {} - triggering immediate export",
            plugin_id,
            scan_id
        );

        // Trigger immediate export of this data
        self.export_plugin_data(plugin_id, scan_id, &data_export)
            .await?;

        Ok(())
    }

    /// Export data from a specific plugin immediately
    async fn export_plugin_data(
        &self,
        plugin_id: &str,
        scan_id: &str,
        data_export: &PluginDataExport,
    ) -> crate::plugin::error::PluginResult<()> {
        // Get repository context if available
        let repo_context = self.repository_contexts.get(scan_id);

        log::info!(
            "Exporting data from plugin {} for scan {} (repo: {:?})",
            plugin_id,
            scan_id,
            repo_context.map(|c| &c.repository_path)
        );

        // For now, just log the export - actual format implementation comes in later phases
        log::debug!("Data export timestamp: {:?}", data_export.timestamp);
        log::debug!("Data export metadata: {:?}", data_export.metadata);

        // TODO: In Phase 8-9, implement actual format detection and export logic
        println!(
            "OUTPUT: Plugin {} exported data for scan {}",
            plugin_id, scan_id
        );

        Ok(())
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

    async fn execute(&mut self, _args: &[String]) -> PluginResult<()> {
        if !self.initialized {
            return Err(PluginError::Generic {
                message: "OutputPlugin not initialized".to_string(),
            });
        }

        // Basic execute implementation - will be expanded in later phases
        // For now, as an auto-active plugin, we need to signal completion
        // immediately to prevent scanner manager hang
        let manager = self.notification_manager.as_ref().ok_or_else(|| {
            crate::plugin::error::PluginError::Generic {
                message: "Notification manager not set".to_string(),
            }
        })?;

        use crate::notifications::api::PluginEvent;
        use crate::notifications::event::{Event, PluginEventType};

        let event = Event::Plugin(PluginEvent::with_message(
            PluginEventType::Completed,
            self.plugin_info().name.clone(),
            self.processing_scan_id
                .as_deref()
                .unwrap_or("no-active-scan")
                .to_string(),
            "OutputPlugin completed (minimal lifecycle)".to_string(),
        ));

        let mut notification_manager = manager.lock().await;
        notification_manager.publish(event).await.map_err(|e| {
            crate::plugin::error::PluginError::Generic {
                message: format!("Failed to publish completion event: {}", e),
            }
        })?;

        Ok(())
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        self.initialized = false;
        Ok(())
    }

    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &crate::plugin::args::PluginConfig,
    ) -> PluginResult<()> {
        // Use the new args_parse method that follows the dump plugin pattern
        // This automatically handles --help via PluginArgParser
        self.args_parse(args, config).await
    }
}

// Register this builtin plugin for automatic discovery
builtin!(|| crate::plugin::discovery::DiscoveredPlugin {
    info: OutputPlugin::static_plugin_info(),
    factory: Box::new(|| Box::new(OutputPlugin::new())),
});
