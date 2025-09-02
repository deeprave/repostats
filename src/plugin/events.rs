//! Plugin event publishing utilities
//!
//! This module provides standardized utilities for publishing plugin events
//! to prevent code duplication and ensure consistency across the codebase.

use crate::notifications::api::PluginEvent;
use crate::notifications::event::{Event, PluginEventType};
use crate::plugin::error::{PluginError, PluginResult};

/// Publish a plugin event to the notification system
///
/// This utility function provides a standardized way to publish plugin events,
/// ensuring consistent error handling and logging across all plugins.
///
/// # Arguments
/// * `event_type` - The type of plugin event to publish
/// * `plugin_name` - The name of the plugin publishing the event
/// * `message` - Optional message describing the event
///
/// # Returns
/// * `PluginResult<()>` - Success or error result
///
/// # Example
/// ```rust
/// use crate::plugin::events::publish_plugin_event;
/// use crate::notifications::event::PluginEventType;
///
/// // Publish a completion event
/// publish_plugin_event(
///     PluginEventType::Completed,
///     "dump",
///     "Plugin processing completed successfully"
/// ).await?;
/// ```
pub async fn publish_plugin_event(
    event_type: PluginEventType,
    plugin_name: &str,
    message: &str,
) -> PluginResult<()> {
    use crate::core::services::get_services;

    let services = get_services();
    let mut notification_manager = services.notification_manager().await;

    let event = Event::Plugin(PluginEvent::with_message(
        event_type.clone(),
        plugin_name.to_string(),
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
/// * `message` - Optional message describing the completion
///
/// # Example
/// ```rust
/// publish_plugin_completion_event("dump", "All scanners processed").await?;
/// ```
pub async fn publish_plugin_completion_event(plugin_name: &str, message: &str) -> PluginResult<()> {
    publish_plugin_event(PluginEventType::Completed, plugin_name, message).await
}

/// Publish a plugin error event
///
/// Convenience function for publishing error events.
///
/// # Arguments
/// * `plugin_name` - The name of the plugin that encountered an error
/// * `error_message` - Description of the error
///
/// # Example
/// ```rust
/// publish_plugin_error_event("dump", "Failed to process scanner output").await?;
/// ```
pub async fn publish_plugin_error_event(
    plugin_name: &str,
    error_message: &str,
) -> PluginResult<()> {
    publish_plugin_event(PluginEventType::Error, plugin_name, error_message).await
}
