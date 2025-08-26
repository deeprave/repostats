//! Plugin Context
//!
//! Provides plugins with access to the service registry and runtime environment.
//! Minimal context container focused on service discovery.

use crate::core::services::{get_services, ServiceRegistry};

/// Runtime context provided to plugins for service access
pub struct PluginContext {
    /// Service registry for accessing core services
    services: &'static ServiceRegistry,
}

impl std::fmt::Debug for PluginContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginContext")
            .field("services", &"ServiceRegistry")
            .finish()
    }
}

impl PluginContext {
    /// Create a new plugin context with access to services
    pub fn new() -> Self {
        Self {
            services: get_services(),
        }
    }

    /// Access the service registry
    pub fn services(&self) -> &ServiceRegistry {
        self.services
    }
}

impl Default for PluginContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_context_creation() {
        let context = PluginContext::new();

        // Test that we can access services
        let services = context.services();

        // Services should be the same as the global instance
        let global_services = get_services();
        assert!(std::ptr::eq(services, global_services));
    }

    #[test]
    fn test_plugin_context_default() {
        let context = PluginContext::default();
        let context_new = PluginContext::new();

        // Both should access the same global services
        assert!(std::ptr::eq(context.services(), context_new.services()));
    }

    #[tokio::test]
    async fn test_plugin_context_service_access() {
        let context = PluginContext::new();
        let services = context.services();

        // Test that we can access notification manager through context
        let notification_manager = services.notification_manager().await;
        let subscriber_count = notification_manager.subscriber_count();

        // Should be able to get a count (specific value doesn't matter)
        let _ = subscriber_count;

        // Test that we can access queue manager through context
        let queue_manager = services.queue_manager();

        // Should be able to create publisher and consumer
        let publisher = queue_manager
            .create_publisher("test-context-producer".to_string())
            .unwrap();
        let consumer = queue_manager
            .create_consumer("test-context-consumer".to_string())
            .unwrap();

        assert_eq!(publisher.producer_id(), "test-context-producer");
        assert_eq!(consumer.plugin_name(), "test-context-consumer");
    }

    #[test]
    fn test_plugin_context_debug_formatting() {
        let context = PluginContext::new();
        let debug_str = format!("{:?}", context);

        // Should include the struct name
        assert!(debug_str.contains("PluginContext"));
        assert!(debug_str.contains("services"));
    }
}
