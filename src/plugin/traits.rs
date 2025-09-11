//! Plugin Trait System
//!
//! Core traits and data structures for the plugin system including base Plugin trait,
//! ConsumerPlugin for queue message consumption, and metadata structures.
//!
//! # Plugin Architecture
//!
//! Plugins in this system primarily exist to process data from repository scans.
//! The data flow is: Scanner → Queue → Analysis Plugins → Output Plugins
//!
//! - **Analysis Plugins**: Process scan data to build indexes, detect patterns, etc.
//! - **Output Plugins**: Format and produce final reports (e.g., dump plugin)
//!
//! Most plugins consume messages from the queue (implement ConsumerPlugin trait),
//! but some may operate independently or wait for processed data from other plugins.
//!
//! Plugins do NOT control scanners, manage queues, or handle system functions.

use crate::notifications::api::AsyncNotificationManager;
use crate::plugin::args::PluginConfig;
use crate::plugin::error::PluginResult;
use crate::plugin::types::{PluginInfo, PluginType};
use crate::queue::api::QueueConsumer;
use crate::scanner::types::ScanRequires;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Base plugin trait that all plugins must implement
///
/// This trait defines the core interface for all plugins in the system.
/// Plugins may or may not also implement ConsumerPlugin to process
/// queue messages, depending on their specific functionality.
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata
    fn plugin_info(&self) -> PluginInfo;

    /// Get plugin type
    fn plugin_type(&self) -> PluginType;

    /// Get list of functions this plugin advertises
    fn advertised_functions(&self) -> Vec<String>;

    /// Get scanner requirements for this plugin
    ///
    /// Returns bitflags indicating what data the scanner needs to provide
    /// for this plugin to function properly. Defaults to NONE.
    fn requirements(&self) -> ScanRequires {
        ScanRequires::NONE
    }

    /// Check if this plugin is compatible with the given system API version
    ///
    /// The plugin determines its own compatibility requirements. The default
    /// implementation returns false to force plugins to explicitly implement
    /// their compatibility logic.
    ///
    /// Builtin plugins should use `crate::core::version::get_api_version()` as
    /// their minimum required version. External plugins should set this based
    /// on the API version they were compiled against.
    fn is_compatible(&self, _system_api_version: u32) -> bool {
        false // Force plugins to implement their own compatibility check
    }

    /// Set the notification manager for this plugin
    ///
    /// This should be called before initialize() to inject the notification
    /// manager dependency. Plugins should store this reference internally.
    fn set_notification_manager(&mut self, manager: Arc<Mutex<AsyncNotificationManager>>);

    /// Initialize the plugin
    async fn initialize(&mut self) -> PluginResult<()>;

    /// Execute plugin functionality (for direct execution)
    async fn execute(&mut self, args: &[String]) -> PluginResult<()>;

    /// Clean up plugin resources
    async fn cleanup(&mut self) -> PluginResult<()>;

    /// Parse plugin-specific command line arguments with configuration context
    async fn parse_plugin_arguments(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()>;

    /// Attempt to get this plugin as a ConsumerPlugin if it implements that trait
    /// Returns None if this plugin is not a ConsumerPlugin
    fn as_consumer_plugin(&mut self) -> Option<&mut dyn ConsumerPlugin> {
        None // Default implementation - plugins that implement ConsumerPlugin should override this
    }
}

/// Consumer plugin trait for plugins that consume messages from queue
///
/// Plugins that implement this trait can process messages from the global queue.
/// This includes both real-time processors (like dump) that output messages
/// immediately, and accumulating processors that collect and analyze data
/// over time before passing results to output plugins.
///
/// The plugin manages its own consuming lifecycle internally - it starts
/// consuming immediately when the consumer is injected and stops automatically
/// during cleanup or on scan completion/error.
#[async_trait::async_trait]
pub trait ConsumerPlugin: Plugin {
    /// Inject the queue consumer for this plugin
    ///
    /// The plugin should immediately start consuming messages and manage
    /// its own lifecycle internally. The plugin will stop consuming
    /// automatically during cleanup or on scan completion/error.
    async fn inject_consumer(&mut self, consumer: QueueConsumer) -> PluginResult<()>;

    /// Override the default Plugin implementation to return self as ConsumerPlugin
    fn as_consumer_plugin(&mut self) -> Option<&mut dyn ConsumerPlugin>
    where
        Self: Sized,
    {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::error::PluginError;

    // Mock plugin for testing
    #[derive(Debug)]
    struct MockPlugin {
        info: PluginInfo,
        initialized: bool,
        executed: bool,
        cleaned_up: bool,
        args_parsed: bool,
    }

    impl MockPlugin {
        fn new() -> Self {
            Self {
                info: PluginInfo {
                    name: "mock-plugin".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Mock plugin for testing".to_string(),
                    author: "Test Author".to_string(),
                    api_version: 20250101,
                    plugin_type: crate::plugin::types::PluginType::Processing,
                    functions: vec!["test".to_string()],
                    required: ScanRequires::NONE,
                    auto_active: false,
                },
                initialized: false,
                executed: false,
                cleaned_up: false,
                args_parsed: false,
            }
        }
    }

    #[async_trait::async_trait]
    impl Plugin for MockPlugin {
        fn plugin_info(&self) -> PluginInfo {
            self.info.clone()
        }

        fn plugin_type(&self) -> PluginType {
            PluginType::Processing
        }

        fn advertised_functions(&self) -> Vec<String> {
            vec!["test".to_string()]
        }

        fn set_notification_manager(&mut self, _manager: Arc<Mutex<AsyncNotificationManager>>) {
            // Mock implementation - just ignore the manager
        }

        async fn initialize(&mut self) -> PluginResult<()> {
            self.initialized = true;
            Ok(())
        }

        async fn execute(&mut self, _args: &[String]) -> PluginResult<()> {
            if !self.initialized {
                return Err(PluginError::ExecutionError {
                    plugin_name: self.info.name.clone(),
                    operation: "execute".to_string(),
                    cause: "Plugin not initialized".to_string(),
                });
            }
            self.executed = true;
            Ok(())
        }

        async fn cleanup(&mut self) -> PluginResult<()> {
            self.cleaned_up = true;
            Ok(())
        }

        async fn parse_plugin_arguments(
            &mut self,
            _args: &[String],
            _config: &PluginConfig,
        ) -> PluginResult<()> {
            self.args_parsed = true;
            Ok(())
        }

        fn is_compatible(&self, _system_api_version: u32) -> bool {
            // Mock plugin is always compatible for testing
            true
        }
    }

    // Mock consumer plugin for testing
    #[derive(Debug)]
    struct MockConsumerPlugin {
        base: MockPlugin,
        consuming: bool,
    }

    impl MockConsumerPlugin {
        fn new() -> Self {
            Self {
                base: MockPlugin::new(),
                consuming: false,
            }
        }
    }

    #[async_trait::async_trait]
    impl Plugin for MockConsumerPlugin {
        fn plugin_info(&self) -> PluginInfo {
            self.base.plugin_info()
        }

        fn plugin_type(&self) -> PluginType {
            PluginType::Output
        }

        fn advertised_functions(&self) -> Vec<String> {
            self.base.advertised_functions()
        }

        fn set_notification_manager(&mut self, manager: Arc<Mutex<AsyncNotificationManager>>) {
            self.base.set_notification_manager(manager);
        }

        async fn initialize(&mut self) -> PluginResult<()> {
            self.base.initialize().await
        }

        async fn execute(&mut self, args: &[String]) -> PluginResult<()> {
            self.base.execute(args).await
        }

        async fn cleanup(&mut self) -> PluginResult<()> {
            self.consuming = false;
            self.base.cleanup().await
        }

        async fn parse_plugin_arguments(
            &mut self,
            args: &[String],
            config: &PluginConfig,
        ) -> PluginResult<()> {
            self.base.parse_plugin_arguments(args, config).await
        }

        fn is_compatible(&self, system_api_version: u32) -> bool {
            self.base.is_compatible(system_api_version)
        }
    }

    #[async_trait::async_trait]
    impl ConsumerPlugin for MockConsumerPlugin {
        async fn inject_consumer(&mut self, _consumer: QueueConsumer) -> PluginResult<()> {
            if !self.base.initialized {
                return Err(PluginError::ExecutionError {
                    plugin_name: self.base.info.name.clone(),
                    operation: "inject_consumer".to_string(),
                    cause: "Plugin not initialized".to_string(),
                });
            }
            self.consuming = true;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_plugin_info_creation() {
        let plugin = MockPlugin::new();
        let info = plugin.plugin_info();

        assert_eq!(info.name, "mock-plugin");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.description, "Mock plugin for testing");
        assert_eq!(info.author, "Test Author");
        assert_eq!(info.api_version, 20250101);
    }

    #[tokio::test]
    async fn test_plugin_type_variants() {
        let plugin = MockPlugin::new();
        assert_eq!(plugin.plugin_type(), PluginType::Processing);

        let consumer_plugin = MockConsumerPlugin::new();
        assert_eq!(consumer_plugin.plugin_type(), PluginType::Output);
    }

    #[tokio::test]
    async fn test_plugin_functions() {
        let plugin = MockPlugin::new();
        let functions = plugin.advertised_functions();

        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0], "test");
    }

    #[tokio::test]
    async fn test_plugin_lifecycle() {
        let mut plugin = MockPlugin::new();

        // Test initialization
        assert!(!plugin.initialized);
        plugin.initialize().await.unwrap();
        assert!(plugin.initialized);

        // Test argument parsing
        assert!(!plugin.args_parsed);
        let config = PluginConfig::default();
        plugin
            .parse_plugin_arguments(&["--test".to_string()], &config)
            .await
            .unwrap();
        assert!(plugin.args_parsed);

        // Test execution
        assert!(!plugin.executed);
        plugin.execute(&[]).await.unwrap();
        assert!(plugin.executed);

        // Test cleanup
        assert!(!plugin.cleaned_up);
        plugin.cleanup().await.unwrap();
        assert!(plugin.cleaned_up);
    }

    #[tokio::test]
    async fn test_plugin_execution_requires_initialization() {
        let mut plugin = MockPlugin::new();

        // Try to execute without initialization
        let result = plugin.execute(&[]).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::ExecutionError {
                plugin_name,
                operation,
                cause,
            } => {
                assert_eq!(plugin_name, "mock-plugin");
                assert_eq!(operation, "execute");
                assert_eq!(cause, "Plugin not initialized");
            }
            _ => panic!("Expected ExecutionError"),
        }
    }

    #[tokio::test]
    async fn test_consumer_plugin_trait() {
        let mut consumer_plugin = MockConsumerPlugin::new();

        // Initialize first
        consumer_plugin.initialize().await.unwrap();
        assert!(consumer_plugin.base.initialized);

        // Test consumer creation and consumption start
        let queue_manager = crate::queue::api::get_queue_service();
        let consumer = queue_manager
            .create_consumer("test-plugin".to_string())
            .unwrap();

        assert!(!consumer_plugin.consuming);
        consumer_plugin.inject_consumer(consumer).await.unwrap();
        assert!(consumer_plugin.consuming);
    }

    #[tokio::test]
    async fn test_consumer_plugin_requires_initialization() {
        let mut consumer_plugin = MockConsumerPlugin::new();

        // Try to start consuming without initialization
        let queue_manager = crate::queue::api::get_queue_service();
        let consumer = queue_manager
            .create_consumer("test-plugin".to_string())
            .unwrap();

        let result = consumer_plugin.inject_consumer(consumer).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::ExecutionError {
                operation, cause, ..
            } => {
                assert_eq!(operation, "inject_consumer");
                assert_eq!(cause, "Plugin not initialized");
            }
            _ => panic!("Expected ExecutionError"),
        }
    }

    #[test]
    fn test_plugin_requirements() {
        let plugin = MockPlugin::new();

        // Default implementation should return NONE
        assert_eq!(plugin.requirements(), ScanRequires::NONE);
        assert!(plugin.requirements().is_empty());
    }

    #[test]
    fn test_plugin_info_equality() {
        let info1 = PluginInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Author".to_string(),
            api_version: 20250101,
            plugin_type: crate::plugin::types::PluginType::Processing,
            functions: vec![],
            required: ScanRequires::NONE,
            auto_active: false,
        };

        let info2 = info1.clone();
        assert_eq!(info1, info2);
    }

    // Test removed as PluginFunction struct no longer exists
    // Functions are now represented as simple strings

    #[test]
    fn test_plugin_type_equality() {
        assert_eq!(PluginType::Processing, PluginType::Processing);
        assert_eq!(PluginType::Output, PluginType::Output);
        assert_eq!(PluginType::Notification, PluginType::Notification);

        assert_ne!(PluginType::Processing, PluginType::Output);
    }
}
