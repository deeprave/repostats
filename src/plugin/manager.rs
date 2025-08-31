//! Plugin Manager
//!
//! Central coordinator for plugin lifecycle, compatibility checking, and plugin proxy management.
//! Owns the plugin registry and provides high-level plugin management operations.

use crate::app::cli::command_segmenter::CommandSegment;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::registry::SharedPluginRegistry;
use crate::plugin::types::PluginSource;
use crate::plugin::types::{ActivePluginInfo, PluginFunction, PluginId, PluginInfo};
use crate::plugin::unified_discovery::PluginDiscovery;
use crate::queue::api::{QueueConsumer, QueueManager};
use log::{info, warn};
use std::collections::HashMap;
use toml::Table;

/// Central plugin manager responsible for:
/// - Plugin lifecycle management
/// - Version compatibility checking
/// - Plugin proxy coordination
/// - Plugin registry ownership
pub struct PluginManager {
    /// The plugin registry (owned by this manager)
    registry: SharedPluginRegistry,

    /// Current API version for compatibility checking
    api_version: u32,

    /// Command resolution - which plugin/function to execute
    active_command: Option<String>,

    /// Plugin ID to name mapping for proxy resolution
    plugin_ids: HashMap<PluginId, String>,

    /// Next available plugin ID
    next_id: PluginId,

    /// Currently active plugins (plugins matched to command segments)
    active_plugins: Vec<ActivePluginInfo>,

    /// Plugin-specific TOML configuration sections
    plugin_configs: HashMap<String, Table>,

    /// Queue consumers that have been created but not yet activated
    /// Maps plugin name to its QueueConsumer
    pending_consumers: HashMap<String, QueueConsumer>,

    /// Notification receivers to keep channels alive
    /// Maps subscriber ID to notification receiver
    notification_receivers: HashMap<String, crate::notifications::api::EventReceiver>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(api_version: u32) -> Self {
        Self {
            registry: SharedPluginRegistry::new(),
            api_version,
            active_command: None,
            plugin_ids: HashMap::new(),
            next_id: PluginId::new(1),
            active_plugins: Vec::new(),
            plugin_configs: HashMap::new(),
            pending_consumers: HashMap::new(),
            notification_receivers: HashMap::new(),
        }
    }

    /// Get shared access to the plugin registry
    pub fn registry(&self) -> &SharedPluginRegistry {
        &self.registry
    }

    /// Get current API version
    pub fn api_version(&self) -> u32 {
        self.api_version
    }

    /// Get color configuration setting
    fn get_use_colors_setting(&self) -> bool {
        // Check if stdout is a terminal and no NO_COLOR environment variable
        std::io::IsTerminal::is_terminal(&std::io::stdout()) && std::env::var("NO_COLOR").is_err()
    }

    /// Check if a plugin API version is compatible
    pub fn is_api_compatible(&self, plugin_api_version: u32) -> bool {
        // Same major version (year) is compatible
        self.get_major_version(self.api_version) == self.get_major_version(plugin_api_version)
    }

    /// Get major version (year) from API version
    pub fn get_major_version(&self, api_version: u32) -> u32 {
        api_version / 10000
    }

    /// Validate plugin compatibility before registration
    pub fn validate_plugin_compatibility(&self, plugin_info: &PluginInfo) -> PluginResult<()> {
        if !self.is_api_compatible(plugin_info.api_version) {
            return Err(PluginError::VersionIncompatible {
                message: format!(
                    "Plugin '{}' has incompatible API version {} (expected major version {})",
                    plugin_info.name,
                    plugin_info.api_version,
                    self.get_major_version(self.api_version)
                ),
            });
        }
        Ok(())
    }

    /// Get plugin info by ID (internal)
    async fn get_plugin_info(&self, plugin_id: PluginId) -> PluginResult<PluginInfo> {
        let plugin_name =
            self.plugin_ids
                .get(&plugin_id)
                .ok_or_else(|| PluginError::PluginNotFound {
                    plugin_name: format!("ID:{:?}", plugin_id),
                })?;

        let registry = self.registry.inner().read().await;
        if let Some(plugin) = registry.get_plugin(plugin_name) {
            Ok(plugin.plugin_info())
        } else {
            Err(PluginError::PluginNotFound {
                plugin_name: plugin_name.clone(),
            })
        }
    }

    /// Generate next plugin ID
    fn next_plugin_id(&mut self) -> PluginId {
        let id = self.next_id;
        self.next_id.increment();
        id
    }

    /// Get plugin info by name (internal use only)
    async fn get_plugin_info_by_name(&self, plugin_name: &str) -> PluginResult<PluginInfo> {
        let registry = self.registry.inner().read().await;
        if let Some(plugin) = registry.get_plugin(plugin_name) {
            Ok(plugin.plugin_info())
        } else {
            Err(PluginError::PluginNotFound {
                plugin_name: plugin_name.to_string(),
            })
        }
    }

    /// Resolve command to plugin info
    pub async fn resolve_command(&mut self, command: &str) -> PluginResult<PluginInfo> {
        // For now, assume command == plugin name (simplified)
        self.active_command = Some(command.to_string());
        self.get_plugin_info_by_name(command).await
    }

    /// Discover and initialize plugins with simplified interface
    pub async fn discover_plugins(
        &mut self,
        plugin_dir: Option<&str>,
        exclusions: &[String],
    ) -> PluginResult<()> {
        // Create discovery implementation with our configuration
        let exclusion_strs: Vec<&str> = exclusions.iter().map(|s| s.as_str()).collect();
        let discovery = PluginDiscovery::with_inclusion_config(
            plugin_dir,
            exclusion_strs,
            true, // Always include builtins internally
            true, // Always include externals internally
        );

        log::debug!("Starting plugin discovery with include_builtins=true, include_externals=true");
        let discovered_plugins = discovery.discover_plugins().await?;
        log::debug!(
            "Plugin discovery completed, found {} plugins total",
            discovered_plugins.len()
        );

        // Register discovered plugins and collect auto-active plugins
        let mut auto_active_plugins = Vec::new();
        let mut registry = self.registry.inner().write().await;

        for discovered in discovered_plugins {
            // Validate compatibility
            self.validate_plugin_compatibility(&discovered.info)?;

            // Track auto-active plugins for later activation
            if discovered.info.auto_active {
                auto_active_plugins.push(discovered.info.name.clone());
            }

            // Create plugin instance and register
            match discovered.source {
                PluginSource::Builtin { factory } => {
                    let plugin = factory();
                    registry.register_plugin(plugin)?;
                }
                PluginSource::BuiltinConsumer { factory } => {
                    let consumer_plugin = factory();
                    registry.register_consumer_plugin(consumer_plugin)?;
                }
                PluginSource::External { library_path } => {
                    // TODO: Implement external plugin loading from shared library
                    return Err(PluginError::LoadError {
                        plugin_name: discovered.info.name.clone(),
                        cause: format!(
                            "External plugin loading not yet implemented: {:?}",
                            library_path
                        ),
                    });
                }
            }
        }

        // Auto-activate plugins marked as auto_active
        for plugin_name in &auto_active_plugins {
            info!("Auto-activating plugin: {}", plugin_name);

            // Get plugin info before activating
            let (function_name, plugin_exists) =
                if let Some(plugin) = registry.get_plugin(plugin_name) {
                    let functions = plugin.advertised_functions();
                    let function_name = if let Some(first_function) = functions.first() {
                        first_function.name.clone()
                    } else {
                        "auto".to_string() // Fallback for plugins with no functions
                    };
                    (function_name, true)
                } else if let Some(consumer_plugin) = registry.get_consumer_plugin(plugin_name) {
                    let functions = consumer_plugin.advertised_functions();
                    let function_name = if let Some(first_function) = functions.first() {
                        first_function.name.clone()
                    } else {
                        "auto".to_string()
                    };
                    (function_name, true)
                } else {
                    (String::new(), false)
                };

            if !plugin_exists {
                warn!("Auto-active plugin '{}' not found in registry", plugin_name);
                continue;
            }

            // Activate the plugin in registry
            if let Err(e) = registry.activate_plugin(plugin_name) {
                warn!("Failed to auto-activate plugin '{}': {:?}", plugin_name, e);
                continue;
            }

            // Add to active plugins list
            self.active_plugins.push(ActivePluginInfo {
                plugin_name: plugin_name.clone(),
                function_name,
                args: Vec::new(), // Auto-active plugins have no command line args
            });

            info!(
                "Auto-active plugin '{}' successfully activated",
                plugin_name
            );
        }

        Ok(())
    }

    /// Activate plugins based on command segments
    ///
    /// Matches command segments against discovered plugins and their advertised functions.
    /// This includes matching both primary function names and aliases.
    pub async fn activate_plugins(
        &mut self,
        command_segments: &[CommandSegment],
    ) -> PluginResult<()> {
        // Clear any existing active plugins
        self.active_plugins.clear();

        // Clear active plugins in registry
        self.registry.clear_active_plugins().await;

        // Get all available plugin functions
        let plugin_functions = self.list_plugins_with_functions().await?;

        for segment in command_segments {
            let mut matched = false;

            // Try to find a plugin function that matches this command
            for (plugin_name, functions) in &plugin_functions {
                for function in functions {
                    // Check both primary name and aliases
                    if function.name == segment.command_name
                        || function.aliases.contains(&segment.command_name)
                    {
                        self.active_plugins.push(ActivePluginInfo {
                            plugin_name: plugin_name.clone(),
                            function_name: function.name.clone(),
                            args: segment.args.clone(),
                        });

                        // Also activate in registry
                        self.registry.activate_plugin(plugin_name).await?;

                        matched = true;
                        break;
                    }
                }
                if matched {
                    break;
                }
            }

            if !matched {
                warn!(
                    "PluginManager: No plugin found for command '{}'",
                    segment.command_name
                );
                return Err(PluginError::PluginNotFound {
                    plugin_name: segment.command_name.clone(),
                });
            }
        }

        Ok(())
    }

    /// Get list of all plugins with their advertised functions (helper method)
    async fn list_plugins_with_functions(
        &self,
    ) -> PluginResult<Vec<(String, Vec<PluginFunction>)>> {
        let registry = self.registry.inner().read().await;
        let mut plugin_functions = Vec::new();

        // Get all plugin names and retrieve their functions
        let plugin_names = registry.get_plugin_names();

        for plugin_name in plugin_names {
            // Try to get as standard plugin first
            if let Some(plugin) = registry.get_plugin(&plugin_name) {
                let functions = plugin.advertised_functions();
                plugin_functions.push((plugin_name, functions));
            }
            // Then try as consumer plugin
            else if let Some(plugin) = registry.get_consumer_plugin(&plugin_name) {
                let functions = plugin.advertised_functions();
                plugin_functions.push((plugin_name, functions));
            }
        }

        Ok(plugin_functions)
    }

    /// Get currently active plugins
    pub fn get_active_plugins(&self) -> &[ActivePluginInfo] {
        &self.active_plugins
    }

    /// Get combined requirements from all active plugins
    pub async fn get_combined_requirements(&self) -> crate::scanner::types::ScanRequires {
        use crate::scanner::types::ScanRequires;

        let registry = self.registry.inner().read().await;
        let mut combined = ScanRequires::NONE;

        // Only get requirements from active plugins, not all plugins
        let active_plugin_names = registry.get_active_plugins();

        for plugin_name in active_plugin_names {
            // Try to get as standard plugin first
            if let Some(plugin) = registry.get_plugin(&plugin_name) {
                combined |= plugin.requirements();
            }
            // Then try as consumer plugin
            else if let Some(plugin) = registry.get_consumer_plugin(&plugin_name) {
                combined |= plugin.requirements();
            }
        }

        combined
    }

    /// Set plugin configurations from main TOML config
    ///
    /// Extracts plugin-specific configuration sections from the main TOML config.
    /// Supports both `[plugins.plugin_name]` and `[plugin_name]` section formats.
    pub fn set_plugin_configs(&mut self, main_config: &Table) -> PluginResult<()> {
        // Clear existing plugin configs
        self.plugin_configs.clear();

        // First, check for [plugins] section with nested plugin configs
        if let Some(plugins_section) = main_config.get("plugins") {
            if let Some(plugins_table) = plugins_section.as_table() {
                for (plugin_name, plugin_config) in plugins_table {
                    if let Some(config_table) = plugin_config.as_table() {
                        self.plugin_configs
                            .insert(plugin_name.clone(), config_table.clone());
                    } else {
                        warn!("PluginManager: Plugin config for '{}' in [plugins] section is not a table, ignoring", plugin_name);
                    }
                }
            } else {
                warn!("PluginManager: [plugins] section exists but is not a table, ignoring");
            }
        }

        // Second, check for direct plugin sections [plugin_name]
        // This allows both [plugins.dump] and [dump] to work
        for (key, value) in main_config {
            // Skip the plugins section we already processed
            if key == "plugins" {
                continue;
            }

            // Check if this might be a plugin name by seeing if it's a table
            if let Some(config_table) = value.as_table() {
                // Only treat as plugin config if we don't already have it from [plugins] section
                if !self.plugin_configs.contains_key(key) {
                    // Try to determine if this looks like a plugin config section
                    // For now, we'll be permissive and include any table section
                    self.plugin_configs
                        .insert(key.clone(), config_table.clone());
                }
            }
        }

        Ok(())
    }

    /// Get plugin configuration by plugin name
    ///
    /// Returns the TOML configuration table for the specified plugin,
    /// or None if no configuration was found.
    pub fn get_plugin_config(&self, plugin_name: &str) -> Option<&Table> {
        self.plugin_configs.get(plugin_name)
    }

    /// Check if a plugin has configuration
    pub fn has_plugin_config(&self, plugin_name: &str) -> bool {
        self.plugin_configs.contains_key(plugin_name)
    }

    /// Get all plugin configurations
    pub fn get_all_plugin_configs(&self) -> &HashMap<String, Table> {
        &self.plugin_configs
    }

    /// Initialize active plugins with their configurations and arguments
    ///
    /// This method initializes all currently active plugins by:
    /// 1. Calling plugin.initialize() for each active plugin
    /// 2. Parsing plugin-specific arguments via plugin.parse_plugin_arguments()
    /// 3. Providing plugin-specific TOML configuration (if available)
    ///
    /// Note: Plugins don't currently receive TOML config directly through their interface.
    /// The TOML config is stored in the PluginManager for future use.
    pub async fn initialize_active_plugins(&mut self) -> PluginResult<()> {
        if self.active_plugins.is_empty() {
            return Ok(());
        }

        let mut registry = self.registry.inner().write().await;

        for active_plugin in &self.active_plugins {
            let plugin_name = &active_plugin.plugin_name;
            let args = &active_plugin.args;

            // Try to find plugin in either standard or consumer plugin registry
            let mut plugin_initialized = false;

            // Try standard plugin first
            if let Some(plugin) = registry.get_plugin_mut(plugin_name) {
                // Initialize the plugin
                plugin
                    .initialize()
                    .await
                    .map_err(|e| PluginError::ExecutionError {
                        plugin_name: plugin_name.clone(),
                        operation: "initialize".to_string(),
                        cause: format!("Failed to initialize plugin: {}", e),
                    })?;

                // Parse plugin arguments
                let plugin_config = if let Some(toml_table) = self.get_plugin_config(plugin_name) {
                    crate::plugin::args::PluginConfig::from_toml(
                        self.get_use_colors_setting(),
                        toml_table,
                    )
                } else {
                    crate::plugin::args::PluginConfig::default()
                };
                plugin.parse_plugin_arguments(args, &plugin_config).await?;

                plugin_initialized = true;
            }
            // Try consumer plugin if not found as standard plugin
            else if let Some(plugin) = registry.get_consumer_plugin_mut(plugin_name) {
                // Initialize the plugin
                plugin
                    .initialize()
                    .await
                    .map_err(|e| PluginError::ExecutionError {
                        plugin_name: plugin_name.clone(),
                        operation: "initialize".to_string(),
                        cause: format!("Failed to initialize consumer plugin: {}", e),
                    })?;

                // Parse plugin arguments
                let plugin_config = if let Some(toml_table) = self.get_plugin_config(plugin_name) {
                    crate::plugin::args::PluginConfig::from_toml(
                        self.get_use_colors_setting(),
                        toml_table,
                    )
                } else {
                    crate::plugin::args::PluginConfig::default()
                };
                plugin.parse_plugin_arguments(args, &plugin_config).await?;

                plugin_initialized = true;
            }

            if !plugin_initialized {
                return Err(PluginError::PluginNotFound {
                    plugin_name: plugin_name.clone(),
                });
            }
        }

        Ok(())
    }

    /// Setup plugin consumers for queue processing (internal)
    /// Only creates queue consumers for Processing plugins
    pub async fn setup_plugin_consumers(
        &mut self,
        queue: &std::sync::Arc<QueueManager>,
        plugin_names: &[String],
        plugin_args: &[String],
    ) -> PluginResult<()> {
        for plugin_name in plugin_names {
            // Check plugin type first - only Processing plugins get queue subscribers
            let should_create_consumer = {
                let registry = self.registry.inner().read().await;
                if let Some(plugin) = registry.get_plugin(plugin_name) {
                    let plugin_info = plugin.plugin_info();
                    match plugin_info.plugin_type {
                        crate::plugin::types::PluginType::Processing => true,
                        _ => {
                            info!(
                                "Skipping queue consumer for plugin '{}' (type: {:?}) - only Processing plugins get queue subscribers",
                                plugin_name, plugin_info.plugin_type
                            );
                            false
                        }
                    }
                } else if let Some(consumer_plugin) = registry.get_consumer_plugin(plugin_name) {
                    let plugin_info = consumer_plugin.plugin_info();
                    match plugin_info.plugin_type {
                        crate::plugin::types::PluginType::Processing => true,
                        _ => {
                            info!(
                                "Skipping queue consumer for consumer plugin '{}' (type: {:?}) - only Processing plugins get queue subscribers",
                                plugin_name, plugin_info.plugin_type
                            );
                            false
                        }
                    }
                } else {
                    warn!(
                        "Plugin '{}' not found in registry, skipping consumer creation",
                        plugin_name
                    );
                    false
                }
            };

            if should_create_consumer {
                // Create consumer but don't activate it yet
                let consumer = queue.create_consumer(plugin_name.clone()).map_err(|e| {
                    PluginError::AsyncError {
                        message: e.to_string(),
                    }
                })?;

                // Store the consumer for later activation
                self.pending_consumers.insert(plugin_name.clone(), consumer);
                info!(
                    "Created queue consumer for Processing plugin '{}'",
                    plugin_name
                );
            }

            // Parse plugin arguments while we have the registry lock (for all plugin types)
            let plugin_config = if let Some(toml_table) = self.get_plugin_config(plugin_name) {
                crate::plugin::args::PluginConfig::from_toml(
                    self.get_use_colors_setting(),
                    toml_table,
                )
            } else {
                crate::plugin::args::PluginConfig::default()
            };
            let mut registry = self.registry.inner().write().await;
            if let Some(plugin) = registry.get_plugin_mut(plugin_name) {
                plugin
                    .parse_plugin_arguments(plugin_args, &plugin_config)
                    .await?;
            }
        }

        Ok(())
    }

    /// Setup notification subscribers for active plugins during initialization
    pub async fn setup_plugin_notification_subscribers(&mut self) -> PluginResult<()> {
        use crate::core::services::get_services;
        use crate::notifications::api::EventFilter;

        let services = get_services();
        let mut notification_manager = services.notification_manager().await;

        for active_plugin in &self.active_plugins {
            let subscriber_id = format!("plugin-{}-notifications", active_plugin.plugin_name);
            let source = format!("Plugin-{}", active_plugin.plugin_name);

            // Create notification subscriber for system, queue, and scan messages
            // Plugins need to see system events, queue events, and scan events
            match notification_manager.subscribe(subscriber_id.clone(), EventFilter::All, source) {
                Ok(receiver) => {
                    // Store the receiver to keep the channel alive
                    self.notification_receivers.insert(subscriber_id, receiver);
                }
                Err(e) => {
                    return Err(PluginError::AsyncError {
                        message: format!(
                            "Failed to create notification subscriber for plugin '{}': {}",
                            active_plugin.plugin_name, e
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    /// Setup notification subscriber for plugin manager to receive system events
    pub async fn setup_system_notification_subscriber(&mut self) -> PluginResult<()> {
        use crate::core::services::get_services;
        use crate::notifications::api::{Event, EventFilter, SystemEventType};
        use log::{error, info};

        let services = get_services();
        let mut notification_manager = services.notification_manager().await;

        let subscriber_id = "plugin-manager-system".to_string();
        let source = "PluginManager-System".to_string();

        match notification_manager.subscribe(subscriber_id.clone(), EventFilter::SystemOnly, source)
        {
            Ok(mut receiver) => {
                // Spawn a task to listen for system events
                tokio::spawn(async move {
                    while let Some(event) = receiver.recv().await {
                        if let Event::System(sys_event) = event {
                            if sys_event.event_type == SystemEventType::Startup {
                                info!("PluginManager: Received system startup event, activating plugin consumers");

                                // Get services and plugin manager inside the loop
                                let services = get_services();
                                let mut plugin_manager = services.plugin_manager().await;

                                // Activate plugin consumers
                                if let Err(e) = plugin_manager.activate_plugin_consumers().await {
                                    error!("Failed to activate plugin consumers: {}", e);
                                }
                            }
                        }
                    }
                    info!("PluginManager: System event listener task terminated");
                });

                Ok(())
            }
            Err(e) => Err(PluginError::AsyncError {
                message: format!(
                    "Failed to create system notification subscriber for plugin manager: {}",
                    e
                ),
            }),
        }
    }

    /// Handle system started event by activating plugin consumers
    pub async fn handle_system_started_event(&mut self) -> PluginResult<()> {
        self.activate_plugin_consumers().await?;

        info!("PluginManager: System started event handling completed");
        Ok(())
    }

    /// Activate all pending plugin consumers (called when system started event is received)
    pub async fn activate_plugin_consumers(&mut self) -> PluginResult<()> {
        if self.pending_consumers.is_empty() {
            info!("PluginManager: No pending consumers to activate");
            return Ok(());
        }

        // Move consumers out of pending_consumers for activation
        let consumers_to_activate: Vec<(String, QueueConsumer)> =
            self.pending_consumers.drain().collect();

        let mut registry = self.registry.inner().write().await;

        for (plugin_name, consumer) in consumers_to_activate.into_iter() {
            info!(
                "PluginManager: Activating consumer for plugin '{}'",
                plugin_name
            );

            // Find the consumer plugin and start consuming
            if let Some(plugin) = registry.get_consumer_plugin_mut(&plugin_name) {
                match plugin.start_consuming(consumer).await {
                    Ok(()) => {
                        info!(
                            "PluginManager: Plugin '{}' is now actively consuming from queue",
                            plugin_name
                        );
                    }
                    Err(e) => {
                        return Err(PluginError::AsyncError {
                            message: format!(
                                "Failed to start consuming for plugin '{}': {}",
                                plugin_name, e
                            ),
                        });
                    }
                }
            } else {
                return Err(PluginError::PluginNotFound {
                    plugin_name: plugin_name.clone(),
                });
            }
        }

        Ok(())
    }

    /// Execute the resolved command (when ready)
    pub fn execute(&self) -> PluginResult<()> {
        Ok(())
    }

    /// List plugins with option to include all plugins or just active ones
    pub async fn list_plugins_with_filter(&self, active_only: bool) -> Vec<PluginInfo> {
        let registry = self.registry.inner().read().await;
        let mut plugins = Vec::new();

        // Get plugin info for plugins based on filter
        let plugin_names = if active_only {
            registry.get_active_plugins()
        } else {
            registry.get_plugin_names()
        };

        for plugin_name in &plugin_names {
            // Try regular plugin first, then consumer plugin
            if let Some(plugin) = registry.get_plugin(plugin_name) {
                plugins.push(plugin.plugin_info());
            } else if let Some(consumer_plugin) = registry.get_consumer_plugin(plugin_name) {
                plugins.push(consumer_plugin.plugin_info());
            } else {
                continue; // Plugin not found in either registry
            };
        }

        plugins
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::args::PluginConfig;
    use crate::plugin::traits::*;
    use crate::plugin::types::PluginType;

    // Mock plugin for testing
    #[derive(Debug)]
    struct MockPlugin {
        name: String,
        initialized: bool,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                initialized: false,
            }
        }
    }

    #[async_trait::async_trait]
    impl Plugin for MockPlugin {
        fn plugin_info(&self) -> PluginInfo {
            PluginInfo {
                name: self.name.clone(),
                version: "1.0.0".to_string(),
                description: "Mock plugin".to_string(),
                author: "Test".to_string(),
                api_version: crate::get_plugin_api_version(),
                plugin_type: self.plugin_type(),
                functions: self.advertised_functions(),
                required: self.requirements().bits(),
                auto_active: false,
            }
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
            Ok(())
        }

        async fn cleanup(&mut self) -> PluginResult<()> {
            Ok(())
        }

        async fn parse_plugin_arguments(
            &mut self,
            _args: &[String],
            _config: &PluginConfig,
        ) -> PluginResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_plugin_manager_creation() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        assert_eq!(manager.api_version, crate::get_plugin_api_version());
        assert_eq!(manager.active_command, None);
        assert_eq!(manager.next_id, PluginId(1));
        assert!(manager.plugin_ids.is_empty());
    }

    #[test]
    fn test_plugin_manager_api_compatibility() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        // Same year should be compatible
        assert!(manager.is_api_compatible(20250101));
        assert!(manager.is_api_compatible(20250215));
        assert!(manager.is_api_compatible(20251231));

        // Different years should not be compatible
        assert!(!manager.is_api_compatible(20240101));
        assert!(!manager.is_api_compatible(20260101));
    }

    #[test]
    fn test_plugin_manager_major_version() {
        let manager = PluginManager::new(20250315); // Different version for compatibility test

        assert_eq!(manager.get_major_version(20250101), 2025);
        assert_eq!(manager.get_major_version(20240831), 2024);
        assert_eq!(manager.get_major_version(20261225), 2026);
    }

    #[test]
    fn test_validate_plugin_compatibility() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        // Compatible plugin
        let compatible_info = PluginInfo {
            name: "compatible".to_string(),
            version: "1.0.0".to_string(),
            description: "Compatible plugin".to_string(),
            author: "Test".to_string(),
            api_version: 20250215,
            plugin_type: PluginType::Processing,
            functions: vec![],
            required: 0,
            auto_active: false,
        };

        assert!(manager
            .validate_plugin_compatibility(&compatible_info)
            .is_ok());

        // Incompatible plugin
        let incompatible_info = PluginInfo {
            name: "incompatible".to_string(),
            version: "1.0.0".to_string(),
            description: "Incompatible plugin".to_string(),
            author: "Test".to_string(),
            api_version: 20240101,
            plugin_type: PluginType::Processing,
            functions: vec![],
            required: 0,
            auto_active: false,
        };

        let result = manager.validate_plugin_compatibility(&incompatible_info);
        assert!(result.is_err());

        match result.unwrap_err() {
            PluginError::VersionIncompatible { message } => {
                assert!(message.contains("incompatible"));
                assert!(message.contains("incompatible"));
                assert!(message.contains("20240101"));
                assert!(message.contains("2025"));
            }
            _ => panic!("Expected VersionIncompatible error"),
        }
    }

    #[tokio::test]
    async fn test_plugin_registry_access() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        // Test that we can access the registry
        let registry = manager.registry();
        assert_eq!(registry.plugin_count().await, 0);

        // Register a plugin directly through the registry
        {
            let mut reg = registry.inner().write().await;
            reg.register_plugin(Box::new(MockPlugin::new("test-plugin")))
                .unwrap();
        }

        assert_eq!(registry.plugin_count().await, 1);
        assert!(registry.has_plugin("test-plugin").await);
    }

    #[tokio::test]
    async fn test_plugin_discovery_finds_dump_plugin() {
        let mut manager = PluginManager::new(crate::get_plugin_api_version());

        // Should discover the dump plugin
        manager.discover_plugins(None, &[]).await.unwrap();
    }

    #[tokio::test]
    async fn test_plugin_discovery_with_exclusions() {
        let mut manager = PluginManager::new(crate::get_plugin_api_version());

        let exclusions = vec!["excluded-plugin".to_string()];
        // Should succeed (no plugins to exclude currently)
        manager.discover_plugins(None, &exclusions).await.unwrap();
    }

    #[tokio::test]
    async fn test_list_plugins_empty() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        let all_plugins = manager.list_plugins_with_filter(false).await;
        assert!(all_plugins.is_empty());

        let active_plugins = manager.list_plugins_with_filter(true).await;
        assert!(active_plugins.is_empty());
    }

    #[tokio::test]
    async fn test_list_plugins_with_registered_plugin() {
        let manager = PluginManager::new(crate::get_plugin_api_version());

        // Register a plugin
        {
            let mut registry = manager.registry().inner().write().await;
            registry
                .register_plugin(Box::new(MockPlugin::new("test-plugin")))
                .unwrap();
        }

        let all_plugins = manager.list_plugins_with_filter(false).await;
        assert_eq!(all_plugins.len(), 1);
        assert_eq!(all_plugins[0].name, "test-plugin");
        assert_eq!(all_plugins[0].version, "1.0.0");
        assert_eq!(all_plugins[0].description, "Mock plugin");
        assert_eq!(all_plugins[0].author, "Test");
        assert_eq!(all_plugins[0].functions.len(), 1);
        assert_eq!(all_plugins[0].functions[0].name, "test");

        // Test that newly registered plugins are NOT active by default
        let active_plugins = manager.list_plugins_with_filter(true).await;
        assert_eq!(active_plugins.len(), 0);

        // Test explicit activation
        manager
            .registry()
            .activate_plugin("test-plugin")
            .await
            .unwrap();
        let active_plugins = manager.list_plugins_with_filter(true).await;
        assert_eq!(active_plugins.len(), 1);
        assert_eq!(active_plugins[0].name, "test-plugin");
    }

    #[test]
    fn test_plugin_id_generation() {
        let mut manager = PluginManager::new(crate::get_plugin_api_version());

        let id1 = manager.next_plugin_id();
        let id2 = manager.next_plugin_id();
        let id3 = manager.next_plugin_id();

        assert_eq!(id1, PluginId(1));
        assert_eq!(id2, PluginId(2));
        assert_eq!(id3, PluginId(3));
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn test_plugin_info_structure() {
        let info = PluginInfo {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Test Author".to_string(),
            api_version: 20250101,
            plugin_type: PluginType::Processing,
            functions: vec![PluginFunction {
                name: "main".to_string(),
                description: "Main function".to_string(),
                aliases: vec!["m".to_string()],
            }],
            required: 0, // ScanRequires::NONE
            auto_active: false,
        };

        assert_eq!(info.name, "test-plugin");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.description, "Test plugin");
        assert_eq!(info.author, "Test Author");
        assert_eq!(info.functions.len(), 1);
        assert_eq!(info.functions[0].name, "main");
        assert_eq!(info.required, 0);
        assert!(!info.auto_active);
    }
}
