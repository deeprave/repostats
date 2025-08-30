//! Plugin Test Utilities
//!
//! Common mock plugins and test helpers to eliminate duplication across test modules.

#[cfg(test)]
use crate::plugin::args::PluginConfig;
#[cfg(test)]
use crate::plugin::error::{PluginError, PluginResult};
#[cfg(test)]
use crate::plugin::traits::{ConsumerPlugin, Plugin};
#[cfg(test)]
use crate::plugin::types::{PluginFunction, PluginInfo, PluginType};
#[cfg(test)]
use crate::queue::api::QueueConsumer;
#[cfg(test)]
use crate::scanner::types::ScanRequires;

/// Configurable mock plugin for comprehensive testing
#[cfg(test)]
#[derive(Debug)]
pub struct MockPlugin {
    pub name: String,
    pub info: PluginInfo,
    pub initialized: bool,
    pub executed: bool,
    pub cleaned_up: bool,
    pub args_parsed: bool,
    pub requirements: ScanRequires,
    pub should_fail_initialize: bool,
    pub should_fail_execute: bool,
}

#[cfg(test)]
impl MockPlugin {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            info: PluginInfo {
                name: name.to_string(),
                version: "1.0.0".to_string(),
                description: "Mock plugin for testing".to_string(),
                author: "Test Author".to_string(),
                api_version: 20250101,
            },
            initialized: false,
            executed: false,
            cleaned_up: false,
            args_parsed: false,
            requirements: ScanRequires::NONE,
            should_fail_initialize: false,
            should_fail_execute: false,
        }
    }

    pub fn with_requirements(mut self, requirements: ScanRequires) -> Self {
        self.requirements = requirements;
        self
    }

    pub fn with_failure_modes(mut self, fail_init: bool, fail_execute: bool) -> Self {
        self.should_fail_initialize = fail_init;
        self.should_fail_execute = fail_execute;
        self
    }
}

#[cfg(test)]
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

    fn requirements(&self) -> ScanRequires {
        self.requirements
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        if self.should_fail_initialize {
            return Err(PluginError::ExecutionError {
                plugin_name: self.info.name.clone(),
                operation: "initialize".to_string(),
                cause: "Mock initialization failure".to_string(),
            });
        }
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
        if self.should_fail_execute {
            return Err(PluginError::ExecutionError {
                plugin_name: self.info.name.clone(),
                operation: "execute".to_string(),
                cause: "Mock execution failure".to_string(),
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

/// Mock consumer plugin for testing ConsumerPlugin trait
#[cfg(test)]
#[derive(Debug)]
pub struct MockConsumerPlugin {
    pub base: MockPlugin,
    pub consuming: bool,
    pub should_fail_start: bool,
    pub should_fail_stop: bool,
}

#[cfg(test)]
impl MockConsumerPlugin {
    pub fn new(name: &str) -> Self {
        Self {
            base: MockPlugin::new(name),
            consuming: false,
            should_fail_start: false,
            should_fail_stop: false,
        }
    }

    pub fn with_requirements(mut self, requirements: ScanRequires) -> Self {
        self.base = self.base.with_requirements(requirements);
        self
    }

    pub fn with_consumer_failures(mut self, fail_start: bool, fail_stop: bool) -> Self {
        self.should_fail_start = fail_start;
        self.should_fail_stop = fail_stop;
        self
    }
}

#[cfg(test)]
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

    fn requirements(&self) -> ScanRequires {
        self.base.requirements()
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        self.base.initialize().await
    }

    async fn execute(&mut self, args: &[String]) -> PluginResult<()> {
        self.base.execute(args).await
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        let _ = self.stop_consuming().await;
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

#[cfg(test)]
#[async_trait::async_trait]
impl ConsumerPlugin for MockConsumerPlugin {
    async fn start_consuming(&mut self, _consumer: QueueConsumer) -> PluginResult<()> {
        if self.should_fail_start {
            return Err(PluginError::ExecutionError {
                plugin_name: self.base.info.name.clone(),
                operation: "start_consuming".to_string(),
                cause: "Mock start consuming failure".to_string(),
            });
        }
        self.consuming = true;
        Ok(())
    }

    async fn stop_consuming(&mut self) -> PluginResult<()> {
        if self.should_fail_stop {
            return Err(PluginError::ExecutionError {
                plugin_name: self.base.info.name.clone(),
                operation: "stop_consuming".to_string(),
                cause: "Mock stop consuming failure".to_string(),
            });
        }
        self.consuming = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_plugin_basic_functionality() {
        let mut plugin = MockPlugin::new("test-plugin");

        assert!(!plugin.initialized);
        assert!(!plugin.executed);
        assert!(!plugin.cleaned_up);

        // Test initialization
        let result = plugin.initialize().await;
        assert!(result.is_ok());
        assert!(plugin.initialized);

        // Test execution
        let result = plugin.execute(&["arg1".to_string()]).await;
        assert!(result.is_ok());
        assert!(plugin.executed);

        // Test cleanup
        let result = plugin.cleanup().await;
        assert!(result.is_ok());
        assert!(plugin.cleaned_up);
    }

    #[tokio::test]
    async fn test_mock_plugin_with_requirements() {
        let plugin = MockPlugin::new("test-plugin")
            .with_requirements(ScanRequires::REPOSITORY_INFO | ScanRequires::COMMITS);

        assert!(plugin.requirements().requires_repository_info());
        assert!(plugin.requirements().requires_commits());
        assert!(!plugin.requirements().requires_file_changes());
    }

    #[tokio::test]
    async fn test_mock_plugin_failure_modes() {
        let mut plugin = MockPlugin::new("test-plugin").with_failure_modes(true, false);

        // Should fail to initialize
        let result = plugin.initialize().await;
        assert!(result.is_err());
        assert!(!plugin.initialized);
    }

    #[tokio::test]
    async fn test_mock_consumer_plugin() {
        use crate::queue::api::QueueManager;
        use std::sync::Arc;

        let mut plugin = MockConsumerPlugin::new("consumer-plugin");

        // Initialize first
        let result = plugin.initialize().await;
        assert!(result.is_ok());

        // Test consumer functionality
        let queue_manager = Arc::new(QueueManager::new());
        let consumer = queue_manager
            .create_consumer("test-consumer".to_string())
            .unwrap();

        let result = plugin.start_consuming(consumer).await;
        assert!(result.is_ok());
        assert!(plugin.consuming);

        let result = plugin.stop_consuming().await;
        assert!(result.is_ok());
        assert!(!plugin.consuming);
    }
}
