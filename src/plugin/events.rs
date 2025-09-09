//! Plugin event publishing utilities
//!
//! This module provides standardized utilities for publishing plugin events
//! to prevent code duplication and ensure consistency across the codebase.

use crate::notifications::api::PluginEvent;
use crate::notifications::event::{Event, PluginEventType};
use crate::plugin::error::{PluginError, PluginResult};

/// System-level scan ID constant used for events not associated with a specific scan
pub const SYSTEM_SCAN_ID: &str = "system";

/// Publish a plugin event to the notification system
///
/// This utility function provides a standardized way to publish plugin events,
/// ensuring consistent error handling and logging across all plugins.
///
/// # Arguments
/// * `event_type` - The type of plugin event to publish
/// * `plugin_name` - The name of the plugin publishing the event
/// * `scan_id` - The scan ID associated with the event, or SYSTEM_SCAN_ID for system events
/// * `message` - Optional message describing the event
///
/// # Returns
/// * `PluginResult<()>` - Success or error result
///
/// (Usage examples removed – internal helper, not part of public API surface.)
pub async fn publish_plugin_event(
    event_type: PluginEventType,
    plugin_name: &str,
    scan_id: &str,
    message: &str,
) -> PluginResult<()> {
    use crate::notifications::api::get_notification_service;

    let mut notification_manager = get_notification_service().await;

    let event = Event::Plugin(PluginEvent::with_message(
        event_type.clone(),
        plugin_name.to_string(),
        scan_id.to_string(),
        message.to_string(),
    ));

    notification_manager
        .publish(event)
        .await
        .map_err(|e| PluginError::LoadError {
            plugin_name: plugin_name.to_string(),
            cause: format!("Failed to publish {:?} event: {}", event_type, e),
        })?;

    log::trace!("{}: Published {:?} event", plugin_name, event_type);
    Ok(())
}

/// Publish a plugin completion event
///
/// Convenience function for publishing completion events specifically.
/// This is the most commonly used event type for plugin coordination.
///
/// # Arguments
/// * `plugin_name` - The name of the plugin that completed
/// * `scan_id` - The scan ID associated with the completion, or "system" for system events
/// * `message` - Optional message describing the completion
///
/// (Usage example removed – internal helper.)
pub async fn publish_plugin_completion_event(
    plugin_name: &str,
    scan_id: &str,
    message: &str,
) -> PluginResult<()> {
    publish_plugin_event(PluginEventType::Completed, plugin_name, scan_id, message).await
}

/// Publish a plugin error event
///
/// Convenience function for publishing error events.
///
/// # Arguments
/// * `plugin_name` - The name of the plugin that encountered an error
/// * `scan_id` - The scan ID associated with the error, or "system" for system events
/// * `error_message` - Description of the error
///
/// (Usage example removed – internal helper.)
pub async fn publish_plugin_error_event(
    plugin_name: &str,
    scan_id: &str,
    error_message: &str,
) -> PluginResult<()> {
    publish_plugin_event(PluginEventType::Error, plugin_name, scan_id, error_message).await
}

/// Publish a plugin keep-alive event
///
/// Convenience function for publishing keep-alive events to indicate
/// that a plugin is still active during long-running operations.
///
/// # Arguments
/// * `plugin_name` - The name of the plugin sending the keep-alive signal
/// * `scan_id` - The scan ID associated with the keep-alive, or "system" for system events
/// * `status_message` - Current status or progress message
///
/// (Usage example removed – internal helper.)
pub async fn publish_plugin_keepalive_event(
    plugin_name: &str,
    scan_id: &str,
    status_message: &str,
) -> PluginResult<()> {
    publish_plugin_event(
        PluginEventType::KeepAlive,
        plugin_name,
        scan_id,
        status_message,
    )
    .await
}
