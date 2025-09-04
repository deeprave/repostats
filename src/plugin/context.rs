//! Plugin Context
//!
//! Provides plugins with access to core services and runtime environment.
//! Simple context container that provides service access functions.

/// Runtime context provided to plugins for service access
///
/// Context is stateless - services are accessed through global functions
pub struct PluginContext {}

impl std::fmt::Debug for PluginContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginContext").finish()
    }
}

impl PluginContext {
    /// Create a new plugin context
    pub fn new() -> Self {
        Self {}
    }

    /// Get notification service
    pub async fn notification_service(
        &self,
    ) -> tokio::sync::MutexGuard<'static, crate::notifications::manager::AsyncNotificationManager>
    {
        crate::notifications::api::get_notification_service().await
    }

    /// Get queue service
    pub fn queue_service(&self) -> std::sync::Arc<crate::queue::manager::QueueManager> {
        crate::queue::api::get_queue_service()
    }

    /// Get plugin service
    pub async fn plugin_service(
        &self,
    ) -> tokio::sync::MutexGuard<'static, crate::plugin::manager::PluginManager> {
        crate::plugin::api::get_plugin_service().await
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

        // Context creation should succeed
        format!("{:?}", context);
    }

    #[test]
    fn test_plugin_context_default() {
        let context = PluginContext::default();
        let context_new = PluginContext::new();

        // Both should be equivalent
        assert_eq!(format!("{:?}", context), format!("{:?}", context_new));
    }

    #[tokio::test]
    async fn test_plugin_context_service_access() {
        let context = PluginContext::new();

        // Test that we can access notification manager through context
        let notification_manager = context.notification_service().await;
        let subscriber_count = notification_manager.subscriber_count();

        // Should be able to get a count (specific value doesn't matter)
        let _ = subscriber_count;

        // Test that we can access queue manager through context
        let queue_manager = context.queue_service();

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
    }
}
