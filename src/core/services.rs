//! Service Registry for centralized access to core services
//!
//! See docs/service_registry.md for complete documentation.

use crate::notifications::api::AsyncNotificationManager;
use crate::queue::QueueManager;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;

/// Global service registry instance
#[allow(dead_code)]
pub static SERVICES: LazyLock<ServiceRegistry> = LazyLock::new(ServiceRegistry::new);

/// Centralized registry for all core services
#[allow(dead_code)]
pub struct ServiceRegistry {
    notification_manager: Mutex<AsyncNotificationManager>,
    queue_manager: Arc<QueueManager>,
}

impl ServiceRegistry {
    /// Create a new ServiceRegistry with default services
    fn new() -> Self {
        Self {
            notification_manager: Mutex::new(AsyncNotificationManager::new()),
            queue_manager: Arc::new(QueueManager::new()),
        }
    }

    /// Access notification manager from async context
    #[allow(dead_code)]
    pub async fn notification_manager(
        &self,
    ) -> tokio::sync::MutexGuard<'_, AsyncNotificationManager> {
        self.notification_manager.lock().await
    }

    /// Access queue manager (returns Arc for shared ownership)
    #[allow(dead_code)]
    pub fn queue_manager(&self) -> Arc<QueueManager> {
        Arc::clone(&self.queue_manager)
    }
}

/// Convenience function to access the global service registry
#[allow(dead_code)]
pub fn get_services() -> &'static ServiceRegistry {
    &SERVICES
}
