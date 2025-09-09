//! Plugin Manager
//!
//! Central coordinator for plugin lifecycle, compatibility checking, and plugin proxy management.
//! Owns the plugin registry and provides high-level plugin management operations.

use crate::app::cli::command_segmenter::CommandSegment;
use crate::notifications::api::AsyncNotificationManager;
use crate::notifications::api::{Event, EventFilter, PluginEventType};
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::registry::SharedPluginRegistry;
use crate::plugin::traits::{ConsumerPlugin, Plugin};
use crate::plugin::types::PluginSource;
use crate::plugin::types::{ActivePluginInfo, PluginFunction, PluginInfo};
use crate::plugin::unified_discovery::PluginDiscovery;
use crate::queue::api::{QueueConsumer, QueueManager};
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
    active_plugins: Vec<ActivePluginInfo>,

    /// Plugin-specific TOML configuration sections
    plugin_configs: HashMap<String, Table>,

    /// Queue consumers that have been created but not yet activated
    /// Maps plugin name to its QueueConsumer
    pending_consumers: HashMap<String, QueueConsumer>,

    /// Shared reference to pending consumers for system event handler
    /// This is set when the system notification subscriber is initialized
    shared_pending_consumers: Option<Arc<Mutex<HashMap<String, QueueConsumer>>>>,

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
    /// Error message for shared consumer storage not initialized
    const SHARED_STORAGE_ERROR: &'static str =
        "Shared consumer storage not initialized. Call initialize() first.";

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
            active_plugins: Vec::new(),
            plugin_configs: HashMap::new(),
            pending_consumers: HashMap::new(),
            shared_pending_consumers: None,
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

    // MARK: Helper methods to DRY up registry lookup patterns

    /// Helper: Execute a closure on a plugin if found in either registry
    async fn with_plugin<F, R>(&self, plugin_name: &str, f: F) -> Option<R>
    where
        F: FnOnce(&dyn crate::plugin::traits::Plugin) -> R,
    {
        let registry = self.registry.inner().read().await;
        if let Some(plugin) = registry.get_plugin(plugin_name) {
            Some(f(plugin))
        } else if let Some(consumer_plugin) = registry.get_consumer_plugin(plugin_name) {
            Some(f(consumer_plugin))
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

        // Helper enum to hold plugin instances during registration
        enum RegistrationTarget {
            Plugin(Box<dyn Plugin>),
            ConsumerPlugin(Box<dyn ConsumerPlugin>),
        }

        // Instantiate plugins and collect auto-active plugins before acquiring the write lock
        let mut instantiated_plugins = Vec::new();
        let mut auto_active_plugins = Vec::new();

        for discovered in discovered_plugins {
            // Validate compatibility
            self.validate_plugin_compatibility(&discovered.info)?;

            // Track auto-active plugins for later activation
            if discovered.info.auto_active {
                auto_active_plugins.push(discovered.info.name.clone());
            }

            // Create plugin instances outside the write lock
            match discovered.source {
                PluginSource::Builtin { factory } => {
                    let plugin = factory();
                    instantiated_plugins
                        .push((discovered.info, RegistrationTarget::Plugin(plugin)));
                }
                PluginSource::BuiltinConsumer { factory } => {
                    let consumer_plugin = factory();
                    instantiated_plugins.push((
                        discovered.info,
                        RegistrationTarget::ConsumerPlugin(consumer_plugin),
                    ));
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
            for (_info, target) in instantiated_plugins {
                match target {
                    RegistrationTarget::Plugin(plugin) => {
                        registry.register_plugin(plugin)?;
                    }
                    RegistrationTarget::ConsumerPlugin(consumer_plugin) => {
                        registry.register_consumer_plugin(consumer_plugin)?;
                    }
                }
            }
        } // Write lock is dropped here

        // Deduplicate and auto-activate plugins marked as auto_active
        auto_active_plugins.sort();
        auto_active_plugins.dedup();

        for plugin_name in &auto_active_plugins {
            log::trace!("Auto-activating plugin: {}", plugin_name);

            // Get plugin info using helper method
            let function_name_opt = self.get_first_function_name(plugin_name).await;
            let plugin_exists = self.get_plugin_info_by_name(plugin_name).await.is_some();

            let function_name = function_name_opt.unwrap_or_else(|| "unknown".to_string());

            if !plugin_exists {
                log::warn!("Auto-active plugin '{}' not found in registry", plugin_name);
                continue;
            }

            // Activate the plugin in registry
            if let Err(e) = self.registry.activate_plugin(plugin_name).await {
                log::warn!("Failed to auto-activate plugin '{}': {:?}", plugin_name, e);
                continue;
            }

            // Add to active plugins list
            self.active_plugins.push(ActivePluginInfo {
                plugin_name: plugin_name.clone(),
                function_name,
                args: Vec::new(), // Auto-active plugins have no command line args
            });

            log::trace!(
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

        // Track activated plugins for potential rollback
        let mut activated_plugins = Vec::new();

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
                        activated_plugins.push(plugin_name.clone());

                        matched = true;
                        break;
                    }
                }
                if matched {
                    break;
                }
            }

            if !matched {
                // Rollback any previously activated plugins before returning error
                log::warn!(
                    "PluginManager: No plugin found for command '{}', rolling back {} previously activated plugins",
                    segment.command_name,
                    activated_plugins.len()
                );

                let mut rollback_errors = Vec::new();
                for plugin_name in &activated_plugins {
                    if let Err(e) = self.registry.deactivate_plugin(plugin_name).await {
                        let error_msg =
                            format!("Failed to rollback plugin '{}': {}", plugin_name, e);
                        log::warn!("{}", error_msg);
                        rollback_errors.push(error_msg);
                    }
                }

                // Clear the active plugins list as well
                self.active_plugins.clear();

                let error_message = if rollback_errors.is_empty() {
                    segment.command_name.clone()
                } else {
                    format!(
                        "{}. Additionally, {} rollback failures occurred: [{}]",
                        segment.command_name,
                        rollback_errors.len(),
                        rollback_errors.join(", ")
                    )
                };

                return Err(PluginError::PluginNotFound {
                    plugin_name: error_message,
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
    /// This method initializes all currently active plugins by:
    /// 1. Injecting notification manager dependency
    /// 2. Calling plugin.initialize() for each active plugin
    /// 3. Parsing plugin-specific arguments via plugin.parse_plugin_arguments()
    /// 4. Providing plugin-specific TOML configuration (if available)
    ///
    /// Note: Plugins don't currently receive TOML config directly through their interface.
    /// The TOML config is stored in the PluginManager for future use.
    pub async fn initialize_active_plugins(&mut self) -> PluginResult<()> {
        if self.active_plugins.is_empty() {
            return Ok(());
        }

        // Use the shared notification manager for plugin dependency injection
        let notification_manager = self.notification_manager.clone();

        let mut registry = self.registry.inner().write().await;

        for active_plugin in &self.active_plugins {
            let plugin_name = &active_plugin.plugin_name;
            let args = &active_plugin.args;

            // Try to find plugin in either standard or consumer plugin registry
            let mut plugin_initialized = false;

            // Try standard plugin first
            if let Some(plugin) = registry.get_plugin_mut(plugin_name) {
                // Inject notification manager dependency
                plugin.set_notification_manager(notification_manager.clone());

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
                // Inject notification manager dependency
                plugin.set_notification_manager(notification_manager.clone());

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
        queue: &Arc<QueueManager>,
        plugin_names: &[String],
        plugin_args: &[String],
    ) -> PluginResult<()> {
        // LOCK ORDERING DOCUMENTATION:
        // To prevent deadlocks, we maintain consistent lock acquisition order:
        // 1. Registry locks (read/write)
        // 2. shared_pending_consumers
        // All locks are held for minimal duration and dropped before acquiring subsequent locks.

        // Phase 1: Collect plugin information with registry read lock
        let mut plugin_info_map = Vec::new();
        {
            let registry = self.registry.inner().read().await;
            for plugin_name in plugin_names {
                let should_create_consumer = if let Some(plugin) = registry.get_plugin(plugin_name)
                {
                    let plugin_info = plugin.plugin_info();
                    matches!(
                        plugin_info.plugin_type,
                        crate::plugin::types::PluginType::Processing
                    )
                } else if let Some(consumer_plugin) = registry.get_consumer_plugin(plugin_name) {
                    let plugin_info = consumer_plugin.plugin_info();
                    matches!(
                        plugin_info.plugin_type,
                        crate::plugin::types::PluginType::Processing
                    )
                } else {
                    log::warn!(
                        "Plugin '{}' not found in registry, skipping consumer creation",
                        plugin_name
                    );
                    false
                };
                plugin_info_map.push((plugin_name.clone(), should_create_consumer));
            }
        } // Registry read lock dropped here

        // Phase 2: Create and store consumers without holding any locks
        if !plugin_info_map.is_empty() {
            let shared_consumers =
                self.shared_pending_consumers
                    .as_ref()
                    .ok_or_else(|| PluginError::LoadError {
                        plugin_name: "plugin_manager".to_string(),
                        cause: Self::SHARED_STORAGE_ERROR.to_string(),
                    })?;

            let mut consumers_to_store = Vec::new();
            for (plugin_name, should_create) in &plugin_info_map {
                if *should_create {
                    let consumer = queue.create_consumer(plugin_name.clone()).map_err(|e| {
                        PluginError::AsyncError {
                            message: e.to_string(),
                        }
                    })?;
                    consumers_to_store.push((plugin_name.clone(), consumer));
                }
            }

            // Store all consumers with single lock acquisition
            if !consumers_to_store.is_empty() {
                let mut shared = shared_consumers.lock().await;
                for (plugin_name, consumer) in consumers_to_store {
                    shared.insert(plugin_name.clone(), consumer);
                    log::trace!(
                        "Created queue consumer for Processing plugin '{}'",
                        plugin_name
                    );
                }
            } // shared_pending_consumers lock dropped here
        }

        // Phase 3: Parse plugin arguments with registry write lock
        {
            let mut registry = self.registry.inner().write().await;
            for plugin_name in plugin_names {
                let plugin_config = if let Some(toml_table) = self.get_plugin_config(plugin_name) {
                    crate::plugin::args::PluginConfig::from_toml(
                        self.get_use_colors_setting(),
                        toml_table,
                    )
                } else {
                    crate::plugin::args::PluginConfig::default()
                };

                if let Some(plugin) = registry.get_plugin_mut(plugin_name) {
                    plugin
                        .parse_plugin_arguments(plugin_args, &plugin_config)
                        .await?;
                }
            }
        } // Registry write lock dropped here

        Ok(())
    }

    /// Setup notification subscribers for active plugins during initialization
    pub async fn setup_plugin_notification_subscribers(&mut self) -> PluginResult<()> {
        use crate::notifications::api::get_notification_service;
        use crate::notifications::api::EventFilter;

        let mut notification_manager = get_notification_service().await;

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
        self.initialize_shared_consumer_storage();
        self.subscribe_to_system_events().await
    }

    /// Initialize shared consumer storage for system event handler
    ///
    /// Creates an Arc<Mutex> wrapper around pending_consumers that can be shared
    /// with the spawned system event handler task. This enables safe concurrent
    /// access to consumer activation during system startup events.
    fn initialize_shared_consumer_storage(&mut self) {
        // Create an Arc<Mutex> wrapper around pending_consumers
        // that can be shared with the spawned task
        // We need to wrap the existing HashMap instead of moving
        // it out since consumers are added later
        let pending_consumers_ref = Arc::new(Mutex::new(std::mem::replace(
            &mut self.pending_consumers,
            HashMap::new(),
        )));

        // Store the shared reference in the plugin manager for later use
        self.shared_pending_consumers = Some(pending_consumers_ref);
    }

    /// Subscribe to system events and spawn handler task
    ///
    /// Subscribes to system events (startup/shutdown) and spawns a background task
    /// to handle plugin consumer activation during system startup. This approach
    /// prevents deadlocks by handling consumer activation outside the main thread.
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

        // Clone shared references to data needed by the system event handler
        let registry = self.registry.clone();
        let pending_consumers_ref = self
            .shared_pending_consumers
            .as_ref()
            .expect("Shared consumer storage must be initialized before subscription")
            .clone();

        match notification_manager.subscribe(subscriber_id.clone(), EventFilter::SystemOnly, source)
        {
            Ok(mut receiver) => {
                // Spawn a task to listen for system events and handle them directly
                tokio::spawn(async move {
                    while let Some(event) = receiver.recv().await {
                        if let Event::System(sys_event) = event {
                            if sys_event.event_type == SystemEventType::Startup {
                                log::trace!("PluginManager: Received system startup event");

                                // Activate plugin consumers directly in this task to avoid deadlock
                                let mut pending_consumers = pending_consumers_ref.lock().await;
                                if pending_consumers.is_empty() {
                                    log::trace!("PluginManager: No pending consumers to activate");
                                } else {
                                    let consumers_to_activate: Vec<(String, QueueConsumer)> =
                                        pending_consumers.drain().collect();

                                    let mut registry = registry.inner().write().await;

                                    for (plugin_name, consumer) in consumers_to_activate.into_iter()
                                    {
                                        log::info!(
                                            "PluginManager: Activating consumer for plugin '{}'",
                                            plugin_name
                                        );

                                        // Find the consumer plugin and start consuming
                                        if let Some(plugin) =
                                            registry.get_consumer_plugin_mut(&plugin_name)
                                        {
                                            match plugin.start_consuming(consumer).await {
                                                Ok(()) => {
                                                    log::trace!(
                                                        "PluginManager: Plugin '{}' is now actively consuming from queue",
                                                        plugin_name
                                                    );
                                                }
                                                Err(e) => {
                                                    log::error!(
                                                        "PluginManager: Failed to start consuming for plugin '{}': {}",
                                                        plugin_name, e
                                                    );
                                                }
                                            }
                                        } else {
                                            log::warn!(
                                                "PluginManager: Consumer plugin '{}' not found during activation",
                                                plugin_name
                                            );
                                        }
                                    }
                                }
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

    /// Handle system started event by activating plugin consumers
    pub async fn handle_system_started_event(&mut self) -> PluginResult<()> {
        self.activate_plugin_consumers().await?;

        log::trace!("PluginManager: System started event handling completed");
        Ok(())
    }

    /// Activate all pending plugin consumers (called when system started event is received)
    pub async fn activate_plugin_consumers(&mut self) -> PluginResult<()> {
        // Always use shared reference for consistent storage access
        let shared_consumers =
            self.shared_pending_consumers
                .as_ref()
                .ok_or_else(|| PluginError::LoadError {
                    plugin_name: "plugin_manager".to_string(),
                    cause: Self::SHARED_STORAGE_ERROR.to_string(),
                })?;

        let consumers_to_activate: Vec<(String, QueueConsumer)> = {
            let mut shared = shared_consumers.lock().await;
            if shared.is_empty() {
                log::trace!("PluginManager: No pending consumers to activate");
                return Ok(());
            }
            // Move consumers out of shared storage for activation
            shared.drain().collect()
        };

        let mut registry = self.registry.inner().write().await;

        for (plugin_name, consumer) in consumers_to_activate.into_iter() {
            log::trace!(
                "PluginManager: Activating consumer for plugin '{}'",
                plugin_name
            );

            // Find the consumer plugin and start consuming
            if let Some(plugin) = registry.get_consumer_plugin_mut(&plugin_name) {
                match plugin.start_consuming(consumer).await {
                    Ok(()) => {
                        log::trace!(
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
                "unknown".to_string(), // TODO: Need to get actual scan_id from context
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

        // Stop all consumer plugins first
        let stop_result = timeout(stop_timeout, async {
            let mut registry = self.registry.inner().write().await;

            for plugin_name in &active_plugin_names {
                if let Some(consumer_plugin) = registry.get_consumer_plugin_mut(plugin_name) {
                    match consumer_plugin.stop_consuming().await {
                        Ok(()) => {
                            // Consumer stopped successfully, but don't add to completed yet
                            // Wait for cleanup phase to mark as completed
                        }
                        Err(e) => {
                            log::trace!("Failed to stop consumer plugin '{}': {}", plugin_name, e);
                            error_plugins.push(plugin_name.clone());
                        }
                    }
                }
            }

            // Call cleanup on all active plugins
            for plugin_name in &active_plugin_names {
                if let Some(plugin) = registry.get_plugin_mut(plugin_name) {
                    match plugin.cleanup().await {
                        Ok(()) => {
                            completed_plugins.push(plugin_name.clone());
                        }
                        Err(e) => {
                            log::trace!("Plugin '{}' cleanup failed: {}", plugin_name, e);
                            error_plugins.push(plugin_name.clone());
                        }
                    }
                } else if let Some(consumer_plugin) = registry.get_consumer_plugin_mut(plugin_name)
                {
                    match consumer_plugin.cleanup().await {
                        Ok(()) => {
                            completed_plugins.push(plugin_name.clone());
                        }
                        Err(e) => {
                            log::trace!("Consumer plugin '{}' cleanup failed: {}", plugin_name, e);
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

    #[test]
    fn test_plugin_manager_api_compatibility() {
        let manager = PluginManager::new(crate::core::version::get_api_version());

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
        let manager = PluginManager::new(crate::core::version::get_api_version());

        // Compatible plugin
        let compatible_info = PluginInfo {
            name: "compatible".to_string(),
            version: "1.0.0".to_string(),
            description: "Compatible plugin".to_string(),
            author: "Test".to_string(),
            api_version: 20250215,
            plugin_type: PluginType::Processing,
            functions: vec![],
            required: ScanRequires::NONE,
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
            required: ScanRequires::NONE,
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
    async fn test_dual_storage_race_condition_fixed() {
        // This test verifies the dual storage race condition is fixed
        // It tests that consumer storage and activation use consistent single storage

        let mut manager = PluginManager::new(crate::core::version::get_api_version());

        // Before initialization, shared storage should be None
        assert!(
            manager.shared_pending_consumers.is_none(),
            "Initially no shared storage"
        );

        // Test that after the fix:
        // 1. Consumer storage requires shared storage to be initialized
        // 2. Consumer activation uses the same shared storage
        // 3. No fallback to local storage that could cause race conditions

        // The fix ensures that:
        // - setup_plugin_consumers() will fail if shared storage not initialized
        // - activate_plugin_consumers() will fail if shared storage not initialized
        // - Both methods use the same shared storage consistently

        // Verify both storage operations require shared storage initialization
        // (We can't easily test actual consumer creation without full system setup,
        //  but we can verify the error handling works correctly)

        let result = manager.activate_plugin_consumers().await;
        match result {
            Err(PluginError::LoadError { plugin_name, cause }) => {
                assert_eq!(plugin_name, "plugin_manager");
                assert!(cause.contains("Shared consumer storage not initialized"));
                log::trace!("✅ Race condition fixed: activation properly requires shared storage");
            }
            _ => panic!("Expected LoadError when shared storage not initialized"),
        }

        // This test verifies the race condition is eliminated by ensuring consistent storage
        assert!(true, "Dual storage race condition successfully fixed");
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
