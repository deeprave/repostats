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

/// Global notification service instance
static NOTIFICATION_SERVICE: LazyLock<Arc<Mutex<AsyncNotificationManager>>> = LazyLock::new(|| {
    log::trace!("Initializing notification service");
    Arc::new(Mutex::new(AsyncNotificationManager::new()))
});

/// Stable notification facade for interacting with the global event bus.
#[derive(Clone, Copy, Debug, Default)]
pub struct NotificationService;

/// Access the stable notification facade.
pub fn notification_service() -> NotificationService {
    NotificationService
}

impl NotificationService {
    /// Publish an event through the global notification bus.
    pub async fn publish(self, event: Event) -> Result<(), NotificationError> {
        let mut manager = NOTIFICATION_SERVICE.lock().await;
        manager.publish(event).await
    }

    /// Subscribe to events from the global notification bus.
    pub async fn subscribe(
        self,
        subscriber_id: String,
        filter: EventFilter,
        source: String,
    ) -> Result<EventReceiver, Box<dyn std::error::Error>> {
        let mut manager = NOTIFICATION_SERVICE.lock().await;
        manager.subscribe(subscriber_id, filter, source)
    }

    /// Return the current number of event subscribers.
    pub async fn subscriber_count(self) -> usize {
        let manager = NOTIFICATION_SERVICE.lock().await;
        manager.subscriber_count()
    }

    pub(crate) fn manager_arc(self) -> Arc<Mutex<AsyncNotificationManager>> {
        NOTIFICATION_SERVICE.clone()
    }
}

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
    let guard = NOTIFICATION_SERVICE.lock().await;
    guard
}

/// Get direct Arc reference to notification service for internal system components
///
/// Used by top-level components like PluginManager to get the notification service
/// reference for dependency injection into their sub-components.
pub(crate) fn get_notification_service_arc() -> Arc<Mutex<AsyncNotificationManager>> {
    log::trace!("Getting notification service Arc reference for internal component");
    notification_service().manager_arc()
}
