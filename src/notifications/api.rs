//! Public API for the notification system
//!
//! This module provides the complete public API for the notification system.
//! External modules should import from here rather than directly from internal modules.
//! See docs/notification_system.md for complete documentation.

use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;

// Core event types and enums
pub use crate::notifications::event::{
    Event, EventFilter, PluginEvent, PluginEventType, QueueEvent, QueueEventType, ScanEvent,
    ScanEventType, SystemEvent, SystemEventType,
};

// Manager and utilities
pub use crate::notifications::error::NotificationError;
pub use crate::notifications::manager::{AsyncNotificationManager, EventReceiver};

// Traits and statistics
pub use crate::notifications::traits::{Subscriber, SubscriberStatistics};

/// Global notification service instance
static NOTIFICATION_SERVICE: LazyLock<Arc<Mutex<AsyncNotificationManager>>> = LazyLock::new(|| {
    log::trace!("Initializing notification service");
    Arc::new(Mutex::new(AsyncNotificationManager::new()))
});

/// Access notification service
///
/// Returns a reference to the global notification service that can be used
/// to publish events and manage subscribers. Each call returns the same
/// shared instance.
///
/// # Examples
/// ```no_run
/// # use repostats::notifications::api::{get_notification_service, Event, SystemEvent, SystemEventType};
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut manager = get_notification_service().await;
/// let event = Event::System(SystemEvent::new(SystemEventType::Startup));
/// manager.publish(event).await?;
/// # Ok(())
/// # }
/// ```
pub async fn get_notification_service() -> tokio::sync::MutexGuard<'static, AsyncNotificationManager>
{
    log::trace!("Acquiring notification service lock");
    let guard = NOTIFICATION_SERVICE.lock().await;
    log::trace!("Acquired notification service lock");
    guard
}

/// Get direct Arc reference to notification service for internal system components
///
/// Used by top-level components like PluginManager to get the notification service
/// reference for dependency injection into their sub-components.
pub(crate) fn get_notification_service_arc() -> Arc<Mutex<AsyncNotificationManager>> {
    log::trace!("Getting notification service Arc reference for internal component");
    NOTIFICATION_SERVICE.clone()
}
