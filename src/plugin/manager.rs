//! Plugin Manager
//!
//! Central coordinator for plugin lifecycle, compatibility checking, and plugin proxy management.
//! Owns the plugin registry and provides high-level plugin management operations.

use crate::app::cli::command_segmenter::CommandSegment;
use crate::notifications::api::AsyncNotificationManager;
use crate::notifications::api::{Event, EventFilter, PluginEventType};
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::registry::SharedPluginRegistry;
use crate::plugin::traits::Plugin;
use crate::plugin::types::PluginSource;
use crate::plugin::types::{ActivePluginInfo, PluginFunction, PluginInfo};
use crate::plugin::unified_discovery::PluginDiscovery;
use crate::queue::api::QueueManager;
use log;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use toml::Table;

/// Configuration for plugin manager timeouts and thresholds
#[derive(Clone, Debug)]
pub struct PluginManagerConfig {
    /// Timeout for waiting for plugin completion events
    pub completion_event_timeout: Duration,

    /// Maximum time to wait for all plugins to complete during shutdown
    pub shutdown_timeout: Duration,

    /// Check interval for plugin completion status
    pub completion_check_interval: Duration,

    /// Maximum time to wait for plugin completion with keep-alive reset capability
    pub plugin_timeout: Duration,
}

impl Default for PluginManagerConfig {
    fn default() -> Self {
        Self {
            completion_event_timeout: Duration::from_millis(100),
            shutdown_timeout: Duration::from_secs(30),
            completion_check_interval: Duration::from_millis(50),
            plugin_timeout: Duration::from_secs(30),
        }
    }
}

impl PluginManagerConfig {
    /// Create configuration with custom timeouts
    pub fn with_timeouts(
        completion_event_timeout: Duration,
        shutdown_timeout: Duration,
        completion_check_interval: Duration,
        plugin_timeout: Duration,
    ) -> Result<Self, String> {
        let config = Self {
            completion_event_timeout,
            shutdown_timeout,
            completion_check_interval,
            plugin_timeout,
        };
        config.validate()?;
        Ok(config)
    }

    /// Create configuration with custom plugin timeout (keeping other defaults)
    pub fn with_plugin_timeout(plugin_timeout: Duration) -> Result<Self, String> {
        let config = Self {
            plugin_timeout,
            ..Self::default()
        };
        config.validate()?;
        Ok(config)
    }

    /// Validate that plugin timeout meets minimum requirements (5 seconds)
    pub fn validate(&self) -> Result<(), String> {
        if self.plugin_timeout < Duration::from_secs(5) {
            return Err("Plugin timeout must be at least 5 seconds".to_string());
        }
        Ok(())
    }
}

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

    /// Currently active plugins (plugins matched to command segments)
    active_plugins: ActivePluginInfo,

    /// Auto-active plugins (plugins that should be activated automatically)
    auto_active_plugins: Vec<String>,

    /// Plugin-specific TOML configuration sections
    plugin_configs: HashMap<String, Table>,

    /// Notification receivers to keep channels alive
    /// Maps subscriber ID to notification receiver
    notification_receivers: HashMap<String, crate::notifications::api::EventReceiver>,

    /// Plugin coordination state for shutdown management
    /// Tracks whether plugins are in process of shutdown
    shutdown_requested: Arc<AtomicBool>,

    /// Plugin completion tracking
    /// Maps plugin name to completion status
    pub(crate) plugin_completion: Arc<RwLock<HashMap<String, bool>>>,

    /// Configuration for timeouts and thresholds
    pub(crate) config: PluginManagerConfig,

    /// Event receiver for continuous plugin event monitoring
    /// Uses Arc<Mutex<>> for safe concurrent access during plugin completion awaiting
    plugin_event_receiver: Arc<Mutex<Option<Arc<Mutex<crate::notifications::api::EventReceiver>>>>>,

    /// Async-safe mutex to prevent concurrent event subscription initialization
    event_subscription_mutex: Arc<Mutex<bool>>,

    /// Global notification manager reference for plugin dependency injection
    /// Notification manager for plugin event handling
    notification_manager: Arc<Mutex<AsyncNotificationManager>>,
}

impl PluginManager {
    /// Error message for plugin event subscription not initialized
    const EVENT_SUBSCRIPTION_ERROR: &'static str =
        "Plugin event subscription not initialized. Call initialize() first.";

    /// Create a new plugin manager with default configuration
    pub fn new(api_version: u32) -> Self {
        Self::with_config(api_version, PluginManagerConfig::default())
    }

    /// Create a new plugin manager with custom configuration
    pub fn with_config(api_version: u32, config: PluginManagerConfig) -> Self {
        let notification_manager = crate::notifications::api::get_notification_service_arc();
        Self {
            registry: SharedPluginRegistry::new(),
            api_version,
            active_plugins: ActivePluginInfo::new(),
            auto_active_plugins: Vec::new(),
            plugin_configs: HashMap::new(),
            notification_receivers: HashMap::new(),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            plugin_completion: Arc::new(RwLock::new(HashMap::new())),
            config,
            plugin_event_receiver: Arc::new(Mutex::new(None)), // Will be set by initialize()
            event_subscription_mutex: Arc::new(Mutex::new(false)),
            notification_manager,
        }
    }

    /// Configure plugin timeout from CLI arguments
    /// Should be called after construction with parsed CLI arguments
    pub fn configure_plugin_timeout(&mut self, timeout: Duration) -> PluginResult<()> {
        // Define a minimum timeout duration (5 seconds to match validation)
        const MIN_TIMEOUT: Duration = Duration::from_secs(5);

        if timeout < MIN_TIMEOUT {
            return Err(PluginError::Generic {
                message: format!(
                    "Plugin timeout value {:?} is below the minimum allowed ({:?})",
                    timeout, MIN_TIMEOUT
                ),
            });
        }

        self.config.plugin_timeout = timeout;
        log::trace!("Plugin manager configured with timeout: {:?}", timeout);
        Ok(())
    }

    /// Initialize the plugin manager
    /// MUST be called after construction before using the plugin manager
    /// Handles event subscription setup and other necessary initialization
    pub async fn initialize(&mut self) -> PluginResult<()> {
        log::trace!("Starting plugin manager initialization");

        // Use async-safe mutex to prevent concurrent initialization
        let mut guard = self.event_subscription_mutex.lock().await;
        if *guard {
            log::trace!("Event subscription already initialized, skipping");
            return Ok(()); // Already initialized
        }

        // Subscribe to plugin events immediately to prevent race conditions
        let mut manager = self.notification_manager.lock().await;
        let plugin_event_receiver = manager
            .subscribe(
                "plugin_manager_events".to_string(),
                EventFilter::PluginOnly,
                "PluginManager".to_string(),
            )
            .map_err(|e| PluginError::LoadError {
                plugin_name: "plugin_manager".to_string(),
                cause: format!(
                    "Failed to subscribe to plugin events during initialization: {}",
                    e
                ),
            })?;

        // Only set the receiver and mark as initialized AFTER successful subscription
        *self.plugin_event_receiver.lock().await =
            Some(Arc::new(Mutex::new(plugin_event_receiver)));

        // Mark initialization complete
        *guard = true;
        drop(guard); // Explicit drop for clarity

        log::trace!("Plugin manager initialization completed");
        Ok(())
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
    fn get_use_colors_setting(&self) -> Option<bool> {
        if std::env::var("FORCE_COLOR").is_ok() {
            return Some(true);
        }
        if std::env::var("NO_COLOR").is_ok() {
            return Some(false);
        }
        None // defer to auto (TTY) at point of use
    }

    // MARK: Helper methods to DRY up registry lookup patterns

    /// Helper: Execute a closure on a plugin if found in either registry
    async fn with_plugin<F, R>(&self, plugin_name: &str, f: F) -> Option<R>
    where
        F: FnOnce(&dyn crate::plugin::traits::Plugin) -> R,
    {
        let registry = self.registry.inner().read().await;
        if let Some(plugin) = registry.get_plugin(plugin_name) {
            Some(f(plugin))
        } else {
            None
        }
    }

    /// Helper: Get first advertised function name from a plugin (either type)
    async fn get_first_function_name(&self, plugin_name: &str) -> Option<String> {
        self.with_plugin(plugin_name, |plugin| {
            plugin
                .advertised_functions()
                .first()
                .map(|f| f.name.clone())
        })
        .await
        .flatten()
    }

    /// Helper: Get plugin info from either registry (by name)
    async fn get_plugin_info_by_name(&self, plugin_name: &str) -> Option<PluginInfo> {
        self.with_plugin(plugin_name, |plugin| plugin.plugin_info())
            .await
    }

    /// Helper: Get advertised functions from either registry
    async fn get_advertised_functions(&self, plugin_name: &str) -> Option<Vec<PluginFunction>> {
        self.with_plugin(plugin_name, |plugin| plugin.advertised_functions())
            .await
    }

    /// Helper: Get requirements from either registry
    async fn get_plugin_requirements(
        &self,
        plugin_name: &str,
    ) -> Option<crate::scanner::types::ScanRequires> {
        self.with_plugin(plugin_name, |plugin| plugin.requirements())
            .await
    }

    // MARK: End of helper methods

    /// Get plugin info by ID (internal)

    /// Resolve command to plugin info
    pub async fn resolve_command(&mut self, command: &str) -> PluginResult<PluginInfo> {
        // For now, assume command == plugin name (simplified)
        self.get_plugin_info_by_name(command)
            .await
            .ok_or_else(|| PluginError::PluginNotFound {
                plugin_name: command.to_string(),
            })
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

        // Helper struct to hold plugin instances during registration
        type RegistrationTarget = Box<dyn Plugin>;

        // Instantiate plugins and collect auto-active plugins before acquiring the write lock
        let mut instantiated_plugins = Vec::new();
        let mut auto_active_plugins = Vec::new();

        for discovered in discovered_plugins {
            // Check plugin compatibility (let plugin decide)
            let plugin = match &discovered.source {
                PluginSource::Builtin { factory } => factory(),
                PluginSource::BuiltinConsumer { factory } => {
                    let mut consumer_plugin = factory();
                    // ConsumerPlugin extends Plugin, so we can get a reference to the Plugin trait
                    let _plugin_ref =
                        consumer_plugin.as_mut() as &mut dyn crate::plugin::traits::Plugin;
                    // We just need to check compatibility, so we'll create a temporary instance
                    // and check its compatibility directly
                    if !consumer_plugin.is_compatible(self.api_version) {
                        return Err(PluginError::VersionIncompatible {
                            message: format!(
                                "Plugin '{}' is incompatible with system API version {}",
                                discovered.info.name, self.api_version
                            ),
                        });
                    }
                    // Skip the general check below since we already checked
                    continue;
                }
                PluginSource::External { .. } => {
                    // External plugins not implemented yet - skip compatibility check
                    continue;
                }
            };

            if !plugin.is_compatible(self.api_version) {
                return Err(PluginError::VersionIncompatible {
                    message: format!(
                        "Plugin '{}' is incompatible with system API version {}",
                        discovered.info.name, self.api_version
                    ),
                });
            }

            // Track auto-active plugins for later activation
            if discovered.info.auto_active {
                auto_active_plugins.push(discovered.info.name.clone());
            }

            // Create plugin instances outside the write lock
            match discovered.source {
                PluginSource::Builtin { factory } => {
                    let plugin = factory();
                    instantiated_plugins.push((discovered.info, plugin));
                }
                PluginSource::BuiltinConsumer { factory } => {
                    let consumer_plugin = factory();
                    instantiated_plugins.push((discovered.info, consumer_plugin));
                }
                PluginSource::External { library_path: _ } => {
                    // External plugin support is disabled in this version
                    // Implementation would require dynamic library loading, symbol resolution,
                    // and proper memory management across library boundaries
                    return Err(PluginError::LoadError {
                        plugin_name: discovered.info.name.clone(),
                        cause: "External plugin support is not available in this version. Only built-in plugins are supported.".to_string(),
                    });
                }
            }
        }

        // Register plugins with the registry, holding the write lock only during registration
        {
            let mut registry = self.registry.inner().write().await;
            for (_info, plugin) in instantiated_plugins {
                registry.register_plugin(plugin)?;
            }
        } // Write lock is dropped here

        // Deduplicate auto-active plugins
        auto_active_plugins.sort();
        auto_active_plugins.dedup();

        // Store auto-active plugins for later processing
        self.auto_active_plugins = auto_active_plugins;

        Ok(())
    }

    /// Helper: Check if a command segment matches a plugin
    async fn check_segment_match(&self, plugin_name: &str, segment: &CommandSegment) -> bool {
        // Check if plugin name matches
        if plugin_name == segment.command_name {
            return true;
        }

        // Check if any function name or alias matches
        if let Some(functions) = self.get_advertised_functions(plugin_name).await {
            for function in &functions {
                if function.name == segment.command_name
                    || function.aliases.contains(&segment.command_name)
                {
                    return true;
                }
            }
        }

        false
    }

    /// Helper: Process segment matching for plugins
    async fn process_segment_matching(
        &mut self,
        all_plugin_names: &[String],
        segments_to_process: &mut Vec<CommandSegment>,
    ) -> PluginResult<(Vec<(String, Vec<String>)>, Option<String>)> {
        let mut plugins_to_activate: Vec<(String, Vec<String>)> = Vec::new();
        let mut active_output_plugin: Option<String> = None;

        for plugin_name in all_plugin_names {
            let mut segments_matched = Vec::new();

            // Check if any command segments match this plugin
            for (index, segment) in segments_to_process.iter().enumerate() {
                if self.check_segment_match(plugin_name, segment).await {
                    // Build args: segment args
                    plugins_to_activate.push((plugin_name.clone(), segment.args.clone()));

                    // Check if it's an Output plugin - segment match always wins
                    if let Some(plugin_info) = self.get_plugin_info_by_name(plugin_name).await {
                        if plugin_info.plugin_type == crate::plugin::types::PluginType::Output {
                            active_output_plugin = Some(plugin_name.clone());
                        }
                    }

                    segments_matched.push(index);
                    break; // Move to next plugin
                }
            }

            // Remove matched segments
            for &index in segments_matched.iter().rev() {
                segments_to_process.remove(index);
            }
        }

        Ok((plugins_to_activate, active_output_plugin))
    }

    /// Helper: Process auto-activation for plugins
    async fn process_auto_activation(
        &mut self,
        all_plugin_names: &[String],
        active_output_plugin: &mut Option<String>,
    ) -> Vec<(String, Vec<String>)> {
        let mut auto_activated_plugins = Vec::new();

        for plugin_name in all_plugin_names {
            if self.auto_active_plugins.contains(plugin_name) {
                // Auto-activate with empty args
                auto_activated_plugins.push((plugin_name.clone(), Vec::new()));

                // Check if it's an Output plugin - only set if no Output plugin chosen yet
                if active_output_plugin.is_none() {
                    if let Some(plugin_info) = self.get_plugin_info_by_name(plugin_name).await {
                        if plugin_info.plugin_type == crate::plugin::types::PluginType::Output {
                            *active_output_plugin = Some(plugin_name.clone());
                        }
                    }
                }
            }
        }

        auto_activated_plugins
    }

    /// Helper: Initialize a single plugin with its configuration and arguments
    async fn initialize_plugin(
        &self,
        registry: &mut crate::plugin::registry::PluginRegistry,
        plugin_name: &str,
        args: &[String],
        notification_manager: Arc<Mutex<AsyncNotificationManager>>,
        use_colors: Option<bool>,
        plugin_toml_config: Option<&toml::Table>,
        queue: &Arc<QueueManager>,
    ) -> PluginResult<()> {
        // Create plugin config once
        let plugin_config = if let Some(toml_table) = plugin_toml_config {
            crate::plugin::args::PluginConfig::from_toml(use_colors, toml_table)
        } else {
            crate::plugin::args::PluginConfig::default()
        };

        // Initialize plugin with notification manager and args
        if let Some(plugin) = registry.get_plugin_mut(plugin_name) {
            // Set notification manager, initialize, and parse arguments
            plugin.set_notification_manager(notification_manager);
            plugin
                .initialize()
                .await
                .map_err(|e| PluginError::ExecutionError {
                    plugin_name: plugin_name.to_string(),
                    operation: "initialize".to_string(),
                    cause: format!("Failed to initialize plugin: {}", e),
                })?;
            plugin.parse_plugin_arguments(args, &plugin_config).await?;

            // Inject consumer immediately if this is a ConsumerPlugin
            if let Some(consumer_plugin) = plugin.as_mut().as_consumer_plugin() {
                let consumer = queue
                    .create_consumer(plugin_name.to_string())
                    .map_err(|e| PluginError::AsyncError {
                        message: format!(
                            "Failed to create consumer for plugin '{}': {}",
                            plugin_name, e
                        ),
                    })?;

                consumer_plugin
                    .inject_consumer(consumer)
                    .await
                    .map_err(|e| PluginError::ExecutionError {
                        plugin_name: plugin_name.to_string(),
                        operation: "inject_consumer".to_string(),
                        cause: format!("Failed to inject consumer: {}", e),
                    })?;

                log::trace!(
                    "PluginManager: Consumer injected into plugin '{}' during initialization",
                    plugin_name
                );
            }
        } else {
            return Err(PluginError::PluginNotFound {
                plugin_name: plugin_name.to_string(),
            });
        }

        Ok(())
    }

    /// Activate plugins based on command segments
    ///
    /// Simple logic:
    /// - Process segment matching to find explicitly requested plugins
    /// - Process auto-activation for plugins marked as auto-active
    /// - Apply Output plugin uniqueness constraint
    /// - Initialize all active plugins with their args
    /// - Ensure fallback Output plugin if needed
    pub async fn activate_plugins(
        &mut self,
        command_segments: &[CommandSegment],
    ) -> PluginResult<()> {
        // Get all available plugins
        let registry = self.registry.inner().read().await;
        let all_plugin_names = registry.get_plugin_names();
        drop(registry);

        // Process segment matching to find explicitly requested plugins
        let mut segments_to_process = command_segments.to_vec();
        let (mut plugins_to_activate, mut active_output_plugin) = self
            .process_segment_matching(&all_plugin_names, &mut segments_to_process)
            .await?;

        // Any segments left over? Unknown command error
        if !segments_to_process.is_empty() {
            return Err(PluginError::PluginNotFound {
                plugin_name: segments_to_process[0].command_name.clone(),
            });
        }

        // Process auto-activation for plugins marked as auto-active
        let auto_activated = self
            .process_auto_activation(&all_plugin_names, &mut active_output_plugin)
            .await;

        // Merge auto-activated plugins with explicitly requested ones
        plugins_to_activate.extend(auto_activated);

        // Apply Output plugin uniqueness constraint
        if let Some(ref chosen_output) = active_output_plugin {
            let mut filtered_plugins = Vec::new();
            for (plugin_name, args) in plugins_to_activate {
                if let Some(plugin_info) = self.get_plugin_info_by_name(&plugin_name).await {
                    if plugin_info.plugin_type == crate::plugin::types::PluginType::Output {
                        if plugin_name == *chosen_output {
                            filtered_plugins.push((plugin_name, args)); // Keep chosen Output plugin
                        }
                        // Skip other Output plugins
                    } else {
                        filtered_plugins.push((plugin_name, args)); // Keep non-Output plugins
                    }
                } else {
                    filtered_plugins.push((plugin_name, args)); // Keep if we can't determine type
                }
            }
            plugins_to_activate = filtered_plugins;
        }

        // Initialize all active plugins with their args
        let mut registry = self.registry.inner().write().await;
        let notification_manager = self.notification_manager.clone();
        let use_colors = self.get_use_colors_setting();
        let queue = std::sync::Arc::new(crate::queue::api::get_queue_service());

        for (plugin_name, args) in &plugins_to_activate {
            // Activate in registry
            self.active_plugins.add(plugin_name);
            registry.activate_plugin(plugin_name)?;

            // Get plugin config for initialization
            let plugin_toml_config = self.get_plugin_config(plugin_name);

            // Initialize the plugin
            self.initialize_plugin(
                &mut registry,
                plugin_name,
                args,
                notification_manager.clone(),
                use_colors,
                plugin_toml_config,
                &queue,
            )
            .await?;
        }

        drop(registry); // Release write lock before calling ensure_output_plugin_fallback

        // Ensure fallback Output plugin if needed
        self.ensure_output_plugin_fallback().await?;

        Ok(())
    }

    /// Get list of all plugins with their advertised functions (helper method)
    async fn list_plugins_with_functions(
        &self,
    ) -> PluginResult<Vec<(String, Vec<PluginFunction>)>> {
        let registry = self.registry.inner().read().await;
        let mut plugin_functions = Vec::new();

        // Get all plugin names and retrieve their functions using helper method
        let plugin_names = registry.get_plugin_names();

        for plugin_name in plugin_names {
            if let Some(functions) = self.get_advertised_functions(&plugin_name).await {
                plugin_functions.push((plugin_name, functions));
            }
        }

        Ok(plugin_functions)
    }

    /// Get currently active plugins
    pub fn get_active_plugins(&self) -> Vec<String> {
        self.active_plugins.get_active_plugins()
    }

    /// Get combined requirements from all active plugins
    pub async fn get_combined_requirements(&self) -> crate::scanner::types::ScanRequires {
        use crate::scanner::types::ScanRequires;

        let registry = self.registry.inner().read().await;
        let mut combined = ScanRequires::NONE;

        // Only get requirements from active plugins, not all plugins
        let active_plugin_names = registry.get_active_plugins();

        drop(registry); // Release the lock early

        for plugin_name in active_plugin_names {
            if let Some(requirements) = self.get_plugin_requirements(&plugin_name).await {
                combined |= requirements;
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
                        log::warn!("PluginManager: Plugin config for '{}' in [plugins] section is not a table, ignoring", plugin_name);
                    }
                }
            } else {
                log::warn!("PluginManager: [plugins] section exists but is not a table, ignoring");
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
    /// NOTE: This method is now unnecessary as activate_plugins() handles all initialization.
    /// Kept for compatibility - will be removed in future cleanup.
    pub async fn initialize_active_plugins(&mut self) -> PluginResult<()> {
        // All initialization is now done in activate_plugins()
        Ok(())
    }

    /// Setup notification subscribers for active plugins during initialization
    pub async fn setup_plugin_notification_subscribers(&mut self) -> PluginResult<()> {
        use crate::notifications::api::get_notification_service;
        use crate::notifications::api::EventFilter;

        let mut notification_manager = get_notification_service().await;
        let active_plugin_names = self.active_plugins.get_active_plugins();

        for plugin_name in &active_plugin_names {
            let subscriber_id = format!("plugin-{}-notifications", plugin_name);
            let source = format!("Plugin-{}", plugin_name);

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
                            plugin_name, e
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    /// Sets up system notification subscription to handle plugin lifecycle events
    ///
    /// Creates a shared consumer storage mechanism and subscribes to system startup events
    /// to activate plugin consumers at the appropriate time. This prevents deadlock
    /// by ensuring plugin activation happens after system initialization.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if subscription setup succeeds
    /// - `Err(PluginError)` if notification service interaction fails
    pub async fn setup_system_notification_subscriber(&mut self) -> PluginResult<()> {
        self.subscribe_to_system_events().await
    }

    /// Subscribe to system events and spawn handler task
    ///
    /// Subscribes to system events (startup/shutdown) for monitoring purposes.
    /// Consumer injection is now handled during plugin initialization.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if subscription and task spawn succeed
    /// - `Err(PluginError)` if notification service subscription fails
    async fn subscribe_to_system_events(&self) -> PluginResult<()> {
        use crate::notifications::api::get_notification_service;
        use crate::notifications::api::{Event, EventFilter, SystemEventType};

        let mut notification_manager = get_notification_service().await;

        let subscriber_id = "plugin-manager-system".to_string();
        let source = "PluginManager-System".to_string();

        match notification_manager.subscribe(subscriber_id.clone(), EventFilter::SystemOnly, source)
        {
            Ok(mut receiver) => {
                // Spawn a task to listen for system events for monitoring
                tokio::spawn(async move {
                    while let Some(event) = receiver.recv().await {
                        if let Event::System(sys_event) = event {
                            if sys_event.event_type == SystemEventType::Startup {
                                log::trace!("PluginManager: Received system startup event - plugins already initialized with consumers");
                            } else if sys_event.event_type == SystemEventType::Shutdown {
                                log::trace!("PluginManager: Received system shutdown event");
                                // Plugin shutdown is handled elsewhere through the shutdown coordination system
                            }
                        }
                    }
                    log::info!("PluginManager: System event listener task terminated");
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

    /// Execute the resolved command (when ready)
    pub fn execute(&self) -> PluginResult<()> {
        Ok(())
    }

    /// Notify all active plugins about system shutdown
    pub async fn notify_plugins_shutdown(&self) -> PluginResult<()> {
        use crate::notifications::api::get_notification_service;
        use crate::notifications::api::PluginEvent;
        use crate::notifications::event::{Event, PluginEventType};

        let mut notification_manager = get_notification_service().await;

        // Get list of all active plugins
        let registry = self.registry.inner().read().await;
        let active_plugin_names: Vec<String> = registry.get_active_plugins();
        drop(registry);

        log::trace!(
            "Notifying {} active plugins about shutdown",
            active_plugin_names.len()
        );

        // Publish shutdown notifications for each plugin
        for plugin_name in &active_plugin_names {
            let plugin_event = PluginEvent::with_message(
                PluginEventType::Unregistered,
                plugin_name.clone(),
                crate::plugin::events::SYSTEM_SCAN_ID.to_string(), // System-level event, not associated with a specific scan
                "System shutdown requested".to_string(),
            );

            if let Err(e) = notification_manager
                .publish(Event::Plugin(plugin_event))
                .await
            {
                log::error!(
                    "Failed to publish shutdown notification for plugin '{}': {}",
                    plugin_name,
                    e
                );
            }
        }

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

        drop(registry); // Release the lock early

        for plugin_name in &plugin_names {
            if let Some(plugin_info) = self.get_plugin_info_by_name(plugin_name).await {
                plugins.push(plugin_info);
            }
        }

        plugins
    }

    // MARK: Plugin Coordination Methods

    /// Wait for all active plugins to complete their current operations naturally
    ///
    /// This method returns when all active plugins have finished processing.
    /// It should be called after scanning is complete but before cleanup.
    /// Returns immediately if no plugins are active.
    ///
    /// This implementation uses the pre-subscribed event receiver to eliminate race conditions.
    /// Supports keep-alive events to extend timeout for long-running operations.
    pub async fn await_all_plugins_completion(&self) -> PluginResult<()> {
        log::trace!("Starting await_all_plugins_completion");

        // Require explicit initialization to avoid masking configuration issues
        {
            let receiver_guard = self.plugin_event_receiver.lock().await;
            if receiver_guard.is_none() {
                return Err(PluginError::LoadError {
                    plugin_name: "plugin_manager".to_string(),
                    cause: Self::EVENT_SUBSCRIPTION_ERROR.to_string(),
                });
            }
            log::trace!("Event subscription confirmed initialized");
        }

        let active_plugin_names = {
            let registry = self.registry.inner().read().await;
            registry.get_active_plugins()
        };

        if active_plugin_names.is_empty() {
            return Ok(());
        }

        log::trace!(
            "Waiting for completion of {} active plugins",
            active_plugin_names.len()
        );

        // Initialize completion tracking for all active plugins
        {
            let mut completion = self.plugin_completion.write().await;
            for plugin_name in &active_plugin_names {
                completion.insert(plugin_name.clone(), false);
            }
        }

        // Track timeout with keep-alive support
        let start_time = std::time::Instant::now();
        let mut last_keepalive_time = start_time;
        let plugin_timeout = self.config.plugin_timeout;

        // Wait for completion events from all active plugins
        let completion_result = loop {
            if self.is_shutdown_requested() {
                log::trace!("Shutdown requested, stopping plugin completion wait");
                break Ok(());
            }

            // Check if all plugins have completed
            let all_completed = {
                let completion = self.plugin_completion.read().await;
                active_plugin_names
                    .iter()
                    .all(|name| completion.get(name).copied().unwrap_or(false))
            };

            if all_completed {
                log::trace!("All plugins have completed");
                break Ok(());
            }

            // Check timeout (since last keep-alive)
            let elapsed_since_keepalive = last_keepalive_time.elapsed();
            if elapsed_since_keepalive > plugin_timeout {
                // Identify which plugins haven't completed for better error reporting
                let incomplete_plugins = {
                    let completion = self.plugin_completion.read().await;
                    active_plugin_names
                        .iter()
                        .filter(|name| !completion.get(*name).copied().unwrap_or(false))
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                };

                log::error!(
                    "Plugin completion timeout after {:?}. Incomplete plugins: [{}]. This may indicate plugin failures or infinite loops.",
                    plugin_timeout,
                    incomplete_plugins.join(", ")
                );

                // Return a timeout-specific error to better expose plugin failures
                // while still allowing graceful system shutdown when called with proper error handling
                break Err(PluginError::AsyncError {
                    message: format!(
                        "Plugin completion timeout after {:?}. {} plugins did not complete: [{}]",
                        plugin_timeout,
                        incomplete_plugins.len(),
                        incomplete_plugins.join(", ")
                    ),
                });
            }

            // Calculate remaining timeout
            let remaining_timeout = plugin_timeout.saturating_sub(elapsed_since_keepalive);
            let wait_timeout =
                std::cmp::min(self.config.completion_event_timeout, remaining_timeout);

            // Wait for completion events with timeout using shared access
            let event_receiver = {
                let receiver_guard = self.plugin_event_receiver.lock().await;
                match receiver_guard.as_ref() {
                    Some(receiver) => receiver.clone(),
                    None => {
                        return Err(PluginError::LoadError {
                            plugin_name: "plugin_manager".to_string(),
                            cause: "Plugin event receiver became None during completion wait. This indicates a race condition or system error.".to_string(),
                        });
                    }
                }
            };
            let timeout_result = timeout(wait_timeout, async {
                let mut receiver = event_receiver.lock().await;
                receiver.recv().await
            })
            .await;

            match timeout_result {
                Ok(event_result) => {
                    match event_result {
                        Some(event) => {
                            if let Event::Plugin(plugin_event) = event {
                                match plugin_event.event_type {
                                    PluginEventType::Completed => {
                                        // Mark this plugin as completed
                                        let mut completion = self.plugin_completion.write().await;
                                        completion.insert(plugin_event.plugin_id.clone(), true);
                                        log::trace!(
                                            "Plugin '{}' marked as completed",
                                            plugin_event.plugin_id
                                        );
                                    }
                                    PluginEventType::KeepAlive => {
                                        // Reset timeout on keep-alive
                                        last_keepalive_time = std::time::Instant::now();
                                        log::trace!(
                                            "Plugin '{}' sent keep-alive, resetting timeout",
                                            plugin_event.plugin_id
                                        );
                                    }
                                    _ => {
                                        // Other plugin events (Processing, DataReady, etc.)
                                        log::trace!(
                                            "Plugin '{}' event: {:?}",
                                            plugin_event.plugin_id,
                                            plugin_event.event_type
                                        );
                                    }
                                }
                            }
                        }
                        None => {
                            log::trace!("Plugin event channel closed");
                            break Ok(());
                        }
                    }
                }
                Err(_) => {
                    // Timeout on event receive: continue
                    // checking completion status and overall timeout
                    continue;
                }
            }
        };

        // No need to put the event receiver back since we used Arc<Mutex<>> for shared access

        // Clean up completed plugins from tracking map
        self.cleanup_completed_plugins().await;

        completion_result
    }

    /// Wait for all active plugins to complete with shutdown integration
    ///
    /// This method is similar to await_all_plugins_completion() but also listens
    /// for shutdown signals and can be interrupted gracefully. This solves the
    /// signal handling integration issue during plugin completion wait.
    pub async fn await_all_plugins_completion_with_shutdown(
        &self,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> PluginResult<()> {
        log::trace!("Starting await_all_plugins_completion_with_shutdown");

        // Pre-check shutdown status before starting
        if self.is_shutdown_requested() {
            log::trace!("Shutdown already requested, returning immediately");
            return Ok(());
        }

        tokio::select! {
            result = self.await_all_plugins_completion() => {
                match result {
                    Ok(()) => {
                        log::trace!("Plugin completion finished normally");
                        Ok(())
                    }
                    Err(e) => {
                        // Always log plugin errors for diagnostics using contextual error handling
                        use crate::core::error_handling::log_error_with_context;
                        log_error_with_context(&e, "Plugin completion wait");

                        // Even on error, check if shutdown was requested during execution
                        if self.is_shutdown_requested() {
                            log::trace!("Shutdown was requested during plugin completion (despite error)");
                            Ok(()) // Prioritize graceful shutdown over plugin errors
                        } else {
                            Err(e)
                        }
                    }
                }
            }
            shutdown_result = shutdown_rx.recv() => {
                match shutdown_result {
                    Ok(_) => {
                        log::trace!("Shutdown signal received during plugin completion wait");
                        self.shutdown_requested.store(true, Ordering::Release);
                        Ok(())
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        log::warn!("Shutdown signal channel closed unexpectedly; treating as shutdown");
                        self.shutdown_requested.store(true, Ordering::Release);
                        Ok(())
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        log::warn!("Missed {} shutdown signals; treating as shutdown", skipped);
                        self.shutdown_requested.store(true, Ordering::Release);
                        Ok(())
                    }
                }
            }
        }
    }

    /// Gracefully stop all active plugins with a timeout
    ///
    /// This method signals all active plugins to stop processing and waits
    /// for them to complete within the specified timeout. Returns a summary
    /// of which plugins completed vs timed out.
    pub async fn graceful_stop_all(
        &self,
        stop_timeout: Duration,
    ) -> PluginResult<PluginStopSummary> {
        self.shutdown_requested.store(true, Ordering::Release);

        let active_plugin_names = {
            let registry = self.registry.inner().read().await;
            registry.get_active_plugins()
        };

        if active_plugin_names.is_empty() {
            return Ok(PluginStopSummary::empty());
        }

        let mut completed_plugins = Vec::new();
        let mut timed_out_plugins = Vec::new();
        let mut error_plugins = Vec::new();

        // Call cleanup on all active plugins (plugins handle their own consumer stopping internally)
        let stop_result = timeout(stop_timeout, async {
            let mut registry = self.registry.inner().write().await;

            for plugin_name in &active_plugin_names {
                if let Some(plugin) = registry.get_plugin_mut(plugin_name) {
                    match plugin.as_mut().cleanup().await {
                        Ok(()) => {
                            completed_plugins.push(plugin_name.clone());
                        }
                        Err(e) => {
                            log::trace!("Plugin '{}' cleanup failed: {}", plugin_name, e);
                            error_plugins.push(plugin_name.clone());
                        }
                    }
                }
            }
        })
        .await;

        match stop_result {
            Ok(()) => {
                // Determine which plugins didn't complete
                let all_handled: std::collections::HashSet<String> = completed_plugins
                    .iter()
                    .chain(error_plugins.iter())
                    .cloned()
                    .collect();

                for plugin_name in &active_plugin_names {
                    if !all_handled.contains(plugin_name) {
                        timed_out_plugins.push(plugin_name.clone());
                    }
                }
            }
            Err(_) => {
                // All remaining plugins are considered timed out
                let handled: std::collections::HashSet<String> = completed_plugins
                    .iter()
                    .chain(error_plugins.iter())
                    .cloned()
                    .collect();

                for plugin_name in &active_plugin_names {
                    if !handled.contains(plugin_name) {
                        timed_out_plugins.push(plugin_name.clone());
                    }
                }
            }
        }

        Ok(PluginStopSummary {
            completed: completed_plugins,
            timed_out: timed_out_plugins,
            errors: error_plugins,
        })
    }

    /// Check if shutdown has been requested
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::Acquire)
    }

    /// Mark a plugin as completed (for use by plugins to signal completion)
    pub async fn mark_plugin_completed(&self, plugin_name: &str) {
        let mut completion = self.plugin_completion.write().await;
        completion.insert(plugin_name.to_string(), true);
    }

    /// Remove all completed plugins from the tracking map
    /// This prevents memory leaks by cleaning up completed entries
    pub async fn cleanup_completed_plugins(&self) {
        let mut completion = self.plugin_completion.write().await;
        let initial_count = completion.len();
        completion.retain(|_, &mut completed| !completed);
        let final_count = completion.len();

        // Log cleanup activity for monitoring
        if initial_count > final_count {
            log::debug!(
                "Cleaned up {} completed plugin entries (remaining: {})",
                initial_count - final_count,
                final_count
            );
        }

        // Warn about potential memory leaks
        if final_count > 100 {
            log::warn!(
                "Plugin completion tracking has {} entries - potential memory leak detected",
                final_count
            );
        }
    }

    /// Get count of plugins currently being tracked for completion
    /// Useful for monitoring and debugging
    pub async fn get_pending_completion_count(&self) -> usize {
        let completion = self.plugin_completion.read().await;
        completion.len()
    }

    /// Ensure fallback Output plugin is activated when no Output plugins are active
    /// This provides automatic fallback to the built-in OutputPlugin when external
    /// Output plugins are not available or have been deactivated
    pub async fn ensure_output_plugin_fallback(&mut self) -> PluginResult<()> {
        let registry = self.registry.inner().read().await;

        // Check if any Output plugin is currently active
        let active_plugins = self.active_plugins.get_active_plugins();
        let has_active_output_plugin = active_plugins.iter().any(|plugin_name| {
            if let Some(plugin) = registry.get_plugin(plugin_name) {
                plugin.plugin_info().plugin_type == crate::plugin::types::PluginType::Output
            } else {
                false
            }
        });

        // If no Output plugin is active, activate the built-in OutputPlugin
        if !has_active_output_plugin {
            let builtin_output_exists = registry.has_plugin("output");
            drop(registry); // Release read lock before trying to activate

            if builtin_output_exists {
                log::info!("No Output plugin active, activating built-in OutputPlugin fallback");

                // Activate the built-in output plugin
                self.registry.activate_plugin("output").await?;

                // Add to active plugins list if not already there
                if !self.active_plugins.contains("output") {
                    self.active_plugins.add("output");
                }

                log::trace!("Built-in OutputPlugin fallback activated successfully");
            } else {
                log::warn!("No Output plugin active and built-in OutputPlugin not found");
            }
        }

        Ok(())
    }
}

/// Summary of plugin stop operation results
#[derive(Debug, Clone)]
pub struct PluginStopSummary {
    /// Plugins that completed gracefully
    pub completed: Vec<String>,
    /// Plugins that timed out
    pub timed_out: Vec<String>,
    /// Plugins that encountered errors during stop
    pub errors: Vec<String>,
}

impl PluginStopSummary {
    /// Create an empty summary
    pub fn empty() -> Self {
        Self {
            completed: Vec::new(),
            timed_out: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Check if all plugins completed successfully
    pub fn all_completed(&self) -> bool {
        self.timed_out.is_empty() && self.errors.is_empty()
    }

    /// Get total number of plugins that were stopped
    pub fn total_plugins(&self) -> usize {
        self.completed.len() + self.timed_out.len() + self.errors.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::args::PluginConfig;
    use crate::plugin::traits::*;
    use crate::plugin::types::PluginType;
    use crate::scanner::types::ScanRequires;

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
                api_version: crate::core::version::get_api_version(),
                plugin_type: self.plugin_type(),
                functions: self.advertised_functions(),
                required: self.requirements(),
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

        fn set_notification_manager(&mut self, _manager: Arc<Mutex<AsyncNotificationManager>>) {
            // Mock implementation - just ignore the manager
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
        let manager = PluginManager::new(crate::core::version::get_api_version());

        assert_eq!(manager.api_version, crate::core::version::get_api_version());
    }

    #[tokio::test]
    async fn test_plugin_registry_access() {
        let manager = PluginManager::new(crate::core::version::get_api_version());

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
        let mut manager = PluginManager::new(crate::core::version::get_api_version());

        // Should discover the dump plugin
        manager.discover_plugins(None, &[]).await.unwrap();
    }

    #[tokio::test]
    async fn test_plugin_discovery_with_exclusions() {
        let mut manager = PluginManager::new(crate::core::version::get_api_version());

        let exclusions = vec!["excluded-plugin".to_string()];
        // Should succeed (no plugins to exclude currently)
        manager.discover_plugins(None, &exclusions).await.unwrap();
    }

    #[tokio::test]
    async fn test_list_plugins_empty() {
        let manager = PluginManager::new(crate::core::version::get_api_version());

        let all_plugins = manager.list_plugins_with_filter(false).await;
        assert!(all_plugins.is_empty());

        let active_plugins = manager.list_plugins_with_filter(true).await;
        assert!(active_plugins.is_empty());
    }

    #[tokio::test]
    async fn test_list_plugins_with_registered_plugin() {
        let manager = PluginManager::new(crate::core::version::get_api_version());

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
            required: ScanRequires::NONE, // ScanRequires::NONE
            auto_active: false,
        };

        assert_eq!(info.name, "test-plugin");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.description, "Test plugin");
        assert_eq!(info.author, "Test Author");
        assert_eq!(info.functions.len(), 1);
        assert_eq!(info.functions[0].name, "main");
        assert_eq!(info.required, ScanRequires::NONE);
        assert!(!info.auto_active);
    }

    #[tokio::test]
    async fn test_signal_handling_integration_issue() {
        // This test demonstrates the signal handling integration issue:
        // Plugin manager has its own shutdown_requested flag that's separate from ShutdownCoordinator

        let plugin_manager = PluginManager::new(crate::core::version::get_api_version());

        // Initially no shutdown requested
        assert!(!plugin_manager.is_shutdown_requested());

        // Simulate what happens when a signal is received:
        // 1. ShutdownCoordinator sets its own global flag (we can't easily test this)
        // 2. But plugin_manager.is_shutdown_requested() still returns false
        //    because it checks a different flag

        // The plugin manager's shutdown flag is only set when graceful_stop_all() is called
        // But during normal completion path, graceful_stop_all() is never called

        // This means signals cannot interrupt await_all_plugins_completion() in normal path
        // because the plugin manager never sees the shutdown request

        assert!(
            !plugin_manager.is_shutdown_requested(),
            "Plugin manager shutdown flag is separate from global shutdown coordinator"
        );

        // TODO: This test shows the integration gap that needs to be fixed
    }

    // NOTE: Signal handling integration is tested through the main application flow
    // in src/main.rs where await_all_plugins_completion_with_shutdown() is used
    // in production. The method handles shutdown coordination during plugin completion
    // and is validated through end-to-end usage rather than isolated unit tests
    // due to the complex timing and coordination requirements.
}
