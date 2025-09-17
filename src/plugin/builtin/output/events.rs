//! Event handling for OutputPlugin
//!
//! This module contains the OutputEventHandler that manages event processing
//! in a separate task to avoid blocking the main plugin execution.

use crate::notifications::api::{
    AsyncNotificationManager, Event, EventReceiver, PluginEvent, PluginEventType, SystemEvent,
    SystemEventType,
};
use crate::plugin::data_export::PluginDataExport;
use crate::plugin::error::PluginResult;
use crate::plugin::registry::SharedPluginRegistry;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};

/// Event handler for OutputPlugin that runs in a separate task
pub struct OutputEventHandler {
    /// Plugin name for identification
    plugin_name: String,
    /// Notification manager for publishing events and keep-alive signals
    notification_manager: Arc<Mutex<AsyncNotificationManager>>,
    /// Plugin registry for checking active plugins (lazy-loaded to avoid deadlock)
    plugin_registry: Option<SharedPluginRegistry>,
    /// Received data exports indexed by (plugin_id, scan_id)
    received_data: Arc<Mutex<HashMap<(String, String), Arc<PluginDataExport>>>>,
    /// Flag indicating if we're currently processing/exporting data
    is_processing_data: Arc<AtomicBool>,
    /// Worker task handle for monitoring failures
    worker_handle: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown signal sender for graceful worker shutdown
    shutdown_sender: broadcast::Sender<()>,
    /// Flag indicating if shutdown signal has been sent
    shutdown_sent: Arc<AtomicBool>,
}

impl OutputEventHandler {
    /// Create a new event handler with owned resources
    pub fn new(
        plugin_name: String,
        notification_manager: Arc<Mutex<AsyncNotificationManager>>,
        received_data: Arc<Mutex<HashMap<(String, String), Arc<PluginDataExport>>>>,
    ) -> Self {
        let (shutdown_sender, _) = broadcast::channel(1);
        Self {
            plugin_name,
            notification_manager,
            plugin_registry: None,
            received_data,
            is_processing_data: Arc::new(AtomicBool::new(false)),
            worker_handle: None,
            shutdown_sender,
            shutdown_sent: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get plugin registry, lazy-loading on first access to avoid deadlock
    async fn get_registry(&mut self) -> &SharedPluginRegistry {
        if self.plugin_registry.is_none() {
            log::debug!("Lazy-loading plugin registry for OutputEventHandler");
            let plugin_manager = crate::plugin::api::get_plugin_service().await;
            self.plugin_registry = Some(plugin_manager.registry().clone());
        }
        self.plugin_registry.as_ref().unwrap()
    }

    /// Run the main event loop - this will be called in a spawned task
    pub async fn run_event_loop(mut self, mut receiver: EventReceiver) -> PluginResult<()> {
        let mut running = true;
        let mut can_exit = false;

        // Spawn the simple worker task and capture handle for monitoring
        let worker_received_data = self.received_data.clone();
        let worker_processing_flag = self.is_processing_data.clone();
        let worker_plugin_name = self.plugin_name.clone();
        let worker_notification_manager = self.notification_manager.clone();
        let worker_shutdown_sender = self.shutdown_sender.clone();

        let worker_handle = tokio::spawn(async move {
            Self::run_simple_worker(
                worker_received_data,
                worker_processing_flag,
                worker_plugin_name,
                worker_notification_manager,
                worker_shutdown_sender,
            )
            .await
        });

        // Store the handle for monitoring
        self.worker_handle = Some(worker_handle);

        while running {
            tokio::select! {
                // Monitor worker task for failures
                worker_result = async {
                    match &mut self.worker_handle {
                        Some(handle) if !handle.is_finished() => {
                            handle.await
                        }
                        _ => std::future::pending().await, // Never resolves if no handle or already finished
                    }
                } => {
                    match worker_result {
                        Ok(()) => {
                            log::warn!("OutputPlugin worker task completed unexpectedly");
                        }
                        Err(e) => {
                            log::error!("OutputPlugin worker task failed: {:?}", e);
                            if e.is_panic() {
                                log::error!("Worker task panicked - OutputPlugin may be in inconsistent state");
                            }
                        }
                    }
                    // Worker failed, continue running but mark it as unavailable
                    self.worker_handle = None;
                }
                // Handle incoming events
                Some(event) = receiver.recv() => {
                    match event {
                        Event::Plugin(plugin_event) => {
                            match self.handle_plugin_event(&plugin_event, &mut can_exit).await {
                                Ok(should_exit) => {
                                    if should_exit {
                                        running = false;
                                    }
                                },
                                Err(e) => {
                                    log::error!("OutputPlugin event handler failed to process plugin event: {:?}", e);
                                    // Continue running - conservative error handling
                                }
                            }
                        }
                        Event::System(system_event) => {
                            match self.handle_system_event(&system_event).await {
                                Ok(should_exit) => {
                                    if should_exit {
                                        running = false;
                                    }
                                },
                                Err(e) => {
                                    log::error!("OutputPlugin event handler failed to process system event: {:?}", e);
                                    // Continue running - conservative error handling
                                }
                            }
                        }
                        _ => {
                            // Ignore other event types (Scan, Queue, etc.)
                        }
                    }
                }
                // Keep-alive timeout - send periodic signals when listening
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    if let Err(e) = self.send_keepalive_signal("Listening for events").await {
                        log::warn!("Failed to send keep-alive signal: {:?}", e);
                    }
                }
                // Channel closed
                else => {
                    log::debug!("OutputPlugin event channel closed");
                    break;
                }
            }
        }

        // Signal worker to shut down gracefully
        if !self.shutdown_sent.swap(true, Ordering::SeqCst) {
            let _ = self.shutdown_sender.send(());
        }

        log::debug!("OutputPlugin event loop completed");
        Ok(())
    }

    /// Handle plugin events (DataReady, Unregistered, DataComplete, etc.)
    async fn handle_plugin_event(
        &mut self,
        event: &PluginEvent,
        can_exit: &mut bool,
    ) -> PluginResult<bool> {
        match event.event_type {
            PluginEventType::DataReady => {
                if let Some(data_export) = &event.data_export {
                    self.handle_data_ready_event(
                        &event.plugin_id,
                        &event.scan_id,
                        data_export.clone(),
                    )
                    .await?;
                }
                Ok(false) // Continue running
            }
            PluginEventType::DataComplete => {
                log::debug!("DataComplete event received for scan {}", event.scan_id);

                // Check if we can exit now
                if *can_exit {
                    let data_map = self.received_data.lock().await;
                    let has_pending_data = !data_map.is_empty();
                    drop(data_map);

                    let is_processing = self.is_processing_data.load(Ordering::SeqCst);

                    if !has_pending_data && !is_processing {
                        log::info!(
                            "All data processed and no active plugins - OutputPlugin can exit"
                        );
                        self.publish_completion("OutputPlugin completed - all data processed")
                            .await?;
                        return Ok(true); // Signal exit
                    }
                }
                Ok(false) // Continue running
            }
            PluginEventType::Unregistered => {
                log::debug!("Plugin {} unregistered", event.plugin_id);

                // Check if all other plugins have unregistered
                let registry = self.get_registry().await;
                let active_plugins = registry.get_active_plugins().await;
                let other_active_plugins: Vec<_> = active_plugins
                    .iter()
                    .filter(|name| *name != &self.plugin_name)
                    .collect();

                if other_active_plugins.is_empty() {
                    log::info!("No other active plugins remaining - setting can_exit flag");
                    *can_exit = true;
                    // Don't exit immediately - wait for data processing to complete
                }

                Ok(false) // Continue running
            }
            _ => {
                Ok(false) // Continue running
            }
        }
    }

    /// Handle system events (Shutdown)
    async fn handle_system_event(&self, event: &SystemEvent) -> PluginResult<bool> {
        match event.event_type {
            SystemEventType::Shutdown => {
                log::info!(
                    "OutputPlugin received shutdown signal - preparing for graceful termination"
                );

                // Signal immediate completion for shutdown scenarios
                if let Err(e) = self
                    .publish_completion("OutputPlugin completed - system shutdown")
                    .await
                {
                    log::warn!(
                        "Failed to publish completion event during shutdown: {:?}",
                        e
                    );
                }

                Ok(true) // Exit immediately
            }
            _ => {
                log::trace!("OutputPlugin handling system event: {:?}", event.event_type);
                Ok(false) // Continue running
            }
        }
    }

    /// Handle DataReady event - process and export data immediately
    async fn handle_data_ready_event(
        &self,
        plugin_id: &str,
        scan_id: &str,
        data_export: Arc<PluginDataExport>,
    ) -> PluginResult<()> {
        log::debug!(
            "OutputPlugin processing DataReady from plugin {} for scan {}",
            plugin_id,
            scan_id
        );

        // Set processing flag and send keep-alive
        self.is_processing_data.store(true, Ordering::SeqCst);
        self.send_keepalive_signal(&format!("Processing data from {}", plugin_id))
            .await?;

        // Store the data
        let key = (plugin_id.to_string(), scan_id.to_string());
        {
            let mut data_map = self.received_data.lock().await;
            data_map.insert(key, data_export.clone());
        }

        // Export the data immediately
        self.export_plugin_data(plugin_id, scan_id, &data_export)
            .await?;

        // Clear processing flag
        self.is_processing_data.store(false, Ordering::SeqCst);

        // Exit checking is now handled by DataComplete events from the worker

        Ok(())
    }

    /// Export data from a specific plugin
    async fn export_plugin_data(
        &self,
        plugin_id: &str,
        scan_id: &str,
        data_export: &PluginDataExport,
    ) -> PluginResult<()> {
        log::info!(
            "Exporting data from plugin {} for scan {}",
            plugin_id,
            scan_id
        );

        // Send keep-alive during export
        self.send_keepalive_signal(&format!("Exporting data from {}", plugin_id))
            .await?;

        // For now, just log the export - actual format implementation comes later
        log::debug!("Data export timestamp: {:?}", data_export.timestamp);
        log::debug!("Data export metadata: {:?}", data_export.metadata);

        // TODO: Implement actual format detection and export logic
        log::info!(
            "Data export completed: plugin='{}' scan='{}' timestamp={:?}",
            plugin_id,
            scan_id,
            data_export.timestamp
        );

        Ok(())
    }

    /// Send a keep-alive signal during active processing
    async fn send_keepalive_signal(&self, status_message: &str) -> PluginResult<()> {
        let mut manager = self.notification_manager.lock().await;

        use crate::notifications::api::PluginEvent;
        use crate::notifications::event::{Event, PluginEventType};

        let event = Event::Plugin(PluginEvent::with_message(
            PluginEventType::KeepAlive,
            self.plugin_name.clone(),
            "global".to_string(), // Generic scan ID for keep-alive
            status_message.to_string(),
        ));

        manager.publish(event).await?;

        Ok(())
    }

    /// Publish completion event
    async fn publish_completion(&self, message: &str) -> PluginResult<()> {
        let mut manager = self.notification_manager.lock().await;

        use crate::notifications::api::PluginEvent;
        use crate::notifications::event::{Event, PluginEventType};

        let event = Event::Plugin(PluginEvent::with_message(
            PluginEventType::Completed,
            self.plugin_name.clone(),
            "global".to_string(),
            message.to_string(),
        ));

        manager.publish(event).await?;

        Ok(())
    }

    /// Simple worker task (placeholder for RS-49 full implementation)
    async fn run_simple_worker(
        received_data: Arc<Mutex<HashMap<(String, String), Arc<PluginDataExport>>>>,
        is_processing_data: Arc<AtomicBool>,
        plugin_name: String,
        notification_manager: Arc<Mutex<AsyncNotificationManager>>,
        shutdown_sender: broadcast::Sender<()>,
    ) {
        let mut shutdown_receiver = shutdown_sender.subscribe();
        let mut processing_shutdown_receiver = shutdown_sender.subscribe();
        let mut wait_shutdown_receiver = shutdown_sender.subscribe();
        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = shutdown_receiver.recv() => {
                    log::debug!("Worker received shutdown signal");
                    break;
                }
                // Check for data to process
                _ = async {
                    let work_item = {
                        let mut data_map = received_data.lock().await;
                        // Pop the first entry if available
                        if let Some(key) = data_map.keys().next().cloned() {
                            let data = data_map.remove(&key);
                            Some((key, data))
                        } else {
                            None
                        }
                    };

                    if let Some(((plugin_id, scan_id), Some(_data_export))) = work_item {
                        // Set processing flag
                        is_processing_data.store(true, Ordering::SeqCst);

                        // Simulate processing with 25 second sleep - but be interruptible
                        log::debug!(
                            "Worker simulating processing for plugin {} scan {}",
                            plugin_id,
                            scan_id
                        );

                        tokio::select! {
                            _ = processing_shutdown_receiver.recv() => {
                                log::debug!("Worker interrupted during processing");
                                is_processing_data.store(false, Ordering::SeqCst);
                                return;
                            }
                            _ = tokio::time::sleep(Duration::from_secs(25)) => {
                                // Processing completed normally
                            }
                        }

                        // Publish DataComplete event BEFORE clearing flag to avoid race condition
                        let mut manager = notification_manager.lock().await;
                        use crate::notifications::api::PluginEvent;
                        use crate::notifications::event::{Event, PluginEventType};

                        let event = Event::Plugin(PluginEvent::with_message(
                            PluginEventType::DataComplete,
                            plugin_name.clone(),
                            scan_id,
                            format!("Completed processing data from {}", plugin_id),
                        ));

                        if let Err(e) = manager.publish(event).await {
                            log::warn!("Failed to publish DataComplete event: {:?}", e);
                        }
                        drop(manager); // Release lock before clearing flag

                        // Clear processing flag AFTER publishing event
                        is_processing_data.store(false, Ordering::SeqCst);
                    } else {
                        // No data available, wait a bit before checking again
                        tokio::select! {
                            _ = wait_shutdown_receiver.recv() => {
                                log::debug!("Worker interrupted during wait");
                                return;
                            }
                            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                                // Continue to next iteration
                            }
                        }
                    }
                } => {}
            }
        }
    }
}

impl Drop for OutputEventHandler {
    fn drop(&mut self) {
        // Send shutdown signal if it hasn't been sent yet
        if !self.shutdown_sent.swap(true, Ordering::SeqCst) {
            let _ = self.shutdown_sender.send(());
        }
    }
}
