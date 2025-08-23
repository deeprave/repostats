//! Service Registry for centralized access to core services
//!
//! See docs/service_registry.md for complete documentation.

use std::sync::{LazyLock, Mutex};
use crate::notifications::api::AsyncNotificationManager;

/// Global service registry instance
pub static SERVICES: LazyLock<ServiceRegistry> = LazyLock::new(|| ServiceRegistry::new());

/// Centralized registry for all core services
pub struct ServiceRegistry {
    notification_manager: Mutex<AsyncNotificationManager>,
}

impl ServiceRegistry {
    /// Create a new ServiceRegistry with default services
    fn new() -> Self {
        Self {
            notification_manager: Mutex::new(AsyncNotificationManager::new()),
        }
    }

    pub fn notification_manager(&self) -> std::sync::MutexGuard<'_, AsyncNotificationManager> {
        self.notification_manager.lock().unwrap()
    }
}

/// Convenience function to access the global service registry
pub fn get_services() -> &'static ServiceRegistry {
    &SERVICES
}