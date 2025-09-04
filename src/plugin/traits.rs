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

use crate::plugin::args::PluginConfig;
use crate::plugin::error::PluginResult;
use crate::plugin::types::{PluginFunction, PluginInfo, PluginType};
use crate::queue::api::QueueConsumer;
use crate::scanner::types::ScanRequires;

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
    fn advertised_functions(&self) -> Vec<PluginFunction>;

    /// Get scanner requirements for this plugin
    ///
    /// Returns bitflags indicating what data the scanner needs to provide
    /// for this plugin to function properly. Defaults to NONE.
    fn requirements(&self) -> ScanRequires {
        ScanRequires::NONE
    }

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
}

/// Consumer plugin trait for plugins that consume messages from queue
///
/// Plugins that implement this trait can process messages from the global queue.
/// This includes both real-time processors (like dump) that output messages
/// immediately, and accumulating processors that collect and analyze data
/// over time before passing results to output plugins.
#[async_trait::async_trait]
pub trait ConsumerPlugin: Plugin {
    /// Start consuming messages from the provided consumer
    ///
    /// This method should spawn a background task to continuously read
    /// messages from the queue and process them according to the plugin's
    /// specific functionality.
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()>;

    /// Stop consuming messages
    async fn stop_consuming(&mut self) -> PluginResult<()>;
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
                    functions: vec![crate::plugin::types::PluginFunction {
                        name: "test".to_string(),
                        description: "Test function".to_string(),
                        aliases: vec![],
                    }],
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

        fn advertised_functions(&self) -> Vec<PluginFunction> {
            vec![PluginFunction {
                name: "test".to_string(),
                description: "Test function".to_string(),
                aliases: vec!["t".to_string()],
            }]
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

        fn advertised_functions(&self) -> Vec<PluginFunction> {
            self.base.advertised_functions()
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
    }

    #[async_trait::async_trait]
    impl ConsumerPlugin for MockConsumerPlugin {
        async fn start_consuming(&mut self, _consumer: QueueConsumer) -> PluginResult<()> {
            if !self.base.initialized {
                return Err(PluginError::ExecutionError {
                    plugin_name: self.base.info.name.clone(),
                    operation: "start_consuming".to_string(),
                    cause: "Plugin not initialized".to_string(),
                });
            }
            self.consuming = true;
            Ok(())
        }

        async fn stop_consuming(&mut self) -> PluginResult<()> {
            self.consuming = false;
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
        assert_eq!(functions[0].name, "test");
        assert_eq!(functions[0].description, "Test function");
        assert_eq!(functions[0].aliases, vec!["t"]);
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
        consumer_plugin.start_consuming(consumer).await.unwrap();
        assert!(consumer_plugin.consuming);

        // Test stop consuming
        consumer_plugin.stop_consuming().await.unwrap();
        assert!(!consumer_plugin.consuming);
    }

    #[tokio::test]
    async fn test_consumer_plugin_requires_initialization() {
        let mut consumer_plugin = MockConsumerPlugin::new();

        // Try to start consuming without initialization
        let queue_manager = crate::queue::api::get_queue_service();
        let consumer = queue_manager
            .create_consumer("test-plugin".to_string())
            .unwrap();

        let result = consumer_plugin.start_consuming(consumer).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::ExecutionError {
                operation, cause, ..
            } => {
                assert_eq!(operation, "start_consuming");
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

    #[test]
    fn test_plugin_function_aliases() {
        let function = PluginFunction {
            name: "primary".to_string(),
            description: "Primary function".to_string(),
            aliases: vec!["p".to_string(), "pri".to_string()],
        };

        assert_eq!(function.aliases.len(), 2);
        assert!(function.aliases.contains(&"p".to_string()));
        assert!(function.aliases.contains(&"pri".to_string()));
    }

    #[test]
    fn test_plugin_type_equality() {
        assert_eq!(PluginType::Processing, PluginType::Processing);
        assert_eq!(PluginType::Output, PluginType::Output);
        assert_eq!(PluginType::Notification, PluginType::Notification);

        assert_ne!(PluginType::Processing, PluginType::Output);
    }
}
