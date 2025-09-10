use crate::app::cli::display::display_plugin_table;
use crate::core::error_handling::log_error_with_context;
use crate::core::error_handling::ContextualError;
use crate::core::validation::ValidationError;
use crate::plugin::error::PluginError;
use log;
use std::fmt;
use toml;

/// Startup operation error types
#[derive(Debug)]
pub enum StartupError {
    /// Logging system initialization failed
    LoggingInitFailed { message: String },
    /// CLI argument validation failed
    ValidationFailed { error: ValidationError },
    /// Plugin operation failed
    PluginFailed { error: PluginError },
    /// Command segmentation failed
    CommandSegmentationFailed { message: String },
    /// Query parameter validation failed
    QueryValidationFailed { message: String },
    /// Display operation failed
    DisplayFailed { message: String },
    /// Configuration error
    ConfigurationError { message: String },
}

impl fmt::Display for StartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartupError::LoggingInitFailed { message } => {
                write!(f, "Failed to initialize logging: {}", message)
            }
            StartupError::ValidationFailed { error } => {
                write!(f, "Invalid CLI arguments: {}", error)
            }
            StartupError::PluginFailed { error } => {
                write!(f, "Plugin operation failed: {}", error)
            }
            StartupError::CommandSegmentationFailed { message } => {
                write!(f, "Command segmentation failed: {}", message)
            }
            StartupError::QueryValidationFailed { message } => {
                write!(f, "Invalid query parameters: {}", message)
            }
            StartupError::DisplayFailed { message } => {
                write!(f, "Display operation failed: {}", message)
            }
            StartupError::ConfigurationError { message } => {
                write!(f, "Configuration error: {}", message)
            }
        }
    }
}

impl std::error::Error for StartupError {}

impl ContextualError for StartupError {
    fn is_user_actionable(&self) -> bool {
        match self {
            // Clear user-actionable errors that users can fix
            StartupError::ValidationFailed { .. } => true,
            StartupError::QueryValidationFailed { .. } => true,
            StartupError::CommandSegmentationFailed { .. } => true,
            StartupError::ConfigurationError { .. } => true,
            // Surface user-actionable plugin errors (like unknown args) instead of hiding
            StartupError::PluginFailed { error } if error.is_user_actionable() => true,

            // System errors users cannot directly fix
            StartupError::LoggingInitFailed { .. }
            | StartupError::PluginFailed { .. }
            | StartupError::DisplayFailed { .. } => false,
        }
    }

    fn user_message(&self) -> Option<&str> {
        match self {
            StartupError::ValidationFailed { error } => {
                // Use the ContextualError trait method for consistency
                error.user_message()
            }
            StartupError::QueryValidationFailed { message } => Some(message),
            StartupError::CommandSegmentationFailed { message } => Some(message),
            StartupError::ConfigurationError { message } => Some(message),
            StartupError::PluginFailed { error } if error.is_user_actionable() => {
                error.user_message()
            }
            _ => None,
        }
    }
}

/// Result type for startup operations
pub type StartupResult<T> = Result<T, StartupError>;

/// Core startup implementation - returns configured ScannerManager
///
/// # Error Handling
/// This function returns a Result to enable proper error handling and testing.
/// All startup errors are wrapped in StartupError variants with appropriate context.
/// The caller is responsible for handling errors, typically by logging and exiting.
///
/// # Returns
/// - `Ok(Some(scanner_manager))` - Successfully configured scanner manager
/// - `Ok(None)` - Successfully initialized but no repositories to scan
/// - `Err(StartupError)` - Startup failed with detailed error information
pub async fn startup(
    command_name: &str,
) -> StartupResult<Option<std::sync::Arc<crate::scanner::api::ScannerManager>>> {
    use super::cli::args::Args;
    use super::cli::command_segmenter::CommandSegmenter;
    use super::cli::initial_args;
    use crate::core::logging::{init_logging, reconfigure_logging};
    use crate::core::strings::title_case;

    // Stage 1: Initial parsing for configuration discovery
    let args = initial_args(command_name)?;
    let command_title = title_case(command_name);
    let use_color = args
        .color
        .unwrap_or_else(|| std::io::IsTerminal::is_terminal(&std::io::stdout()));

    // Check if --plugins flag was provided (early exit)
    if args.plugins {
        log::debug!("--plugins flag detected, will list plugins after discovery");
        // Continue with minimal setup to get to plugin discovery
    }

    // 1.1 Initialize logging
    let log_file_str = args
        .log_file
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());
    if let Err(e) = init_logging(
        args.log_level.as_deref(),
        args.log_format.as_deref(),
        log_file_str.as_deref(),
        use_color,
    ) {
        return Err(StartupError::LoggingInitFailed {
            message: e.to_string(),
        });
    }

    // Stage 2: Command segmentation and config parsing
    let mut final_args = Args::new();
    let toml_config =
        Args::parse_config_file_with_raw_config(&mut final_args, args.config_file.clone()).await;

    let plugin_dir = args
        .plugin_dir
        .clone()
        .or(final_args.plugin_dir.clone())
        .or(dirs::config_dir().map(|d| d.join(command_title).to_string_lossy().to_string()));

    // Stage 2: Reconfigure logging with final values
    let log_level = args.log_level.clone().or(final_args.log_level.clone());
    let log_format = args.log_format.clone().or(final_args.log_format.clone());
    let log_file = log_file_str.clone().or(final_args
        .log_file
        .as_ref()
        .map(|p| p.to_string_lossy().to_string()));
    if let Err(e) = reconfigure_logging(
        log_level.as_deref(),
        log_format.as_deref(),
        log_file.as_deref(),
        use_color,
    ) {
        return Err(StartupError::LoggingInitFailed {
            message: format!(
                "Failed to reconfigure logging system: {}. Configuration: level={:?}, format={:?}, file={:?}, color={}",
                e, log_level, log_format, log_file, use_color
            )
        });
    }

    // Stage 3: Final global args parsing with collected arguments
    Args::parse_from_args(&mut final_args, command_name, &args.global_args, args.color);

    // Validate CLI arguments before proceeding
    if let Err(e) = final_args.validate() {
        return Err(StartupError::ValidationFailed { error: e });
    }

    // Configure plugin manager timeout from CLI args (respects user configuration)
    log::trace!("Configuring plugin manager timeout from CLI args");
    let timeout_duration = final_args.plugin_timeout_duration();
    let mut plugin_manager = crate::plugin::api::get_plugin_service().await;
    log::trace!("Acquired plugin manager lock for timeout configuration");
    if let Err(e) = plugin_manager.configure_plugin_timeout(timeout_duration) {
        return Err(StartupError::PluginFailed { error: e });
    }
    log::debug!("Configured plugin manager timeout: {:?}", timeout_duration);
    log::trace!("Plugin manager timeout configuration completed");
    drop(plugin_manager); // Explicitly release the plugin manager lock before discover_commands

    // Stage 4: Command discovery and segmentation
    log::trace!("Starting command discovery phase");
    let _plugin_dir = plugin_dir.as_deref().or(final_args.plugin_dir.as_deref());
    let commands = discover_commands(_plugin_dir, &args.plugin_exclusions)
        .await
        .map_err(|e| StartupError::PluginFailed { error: e })?;
    log::trace!(
        "Command discovery phase completed, found {} commands",
        commands.len()
    );

    // Check if --plugins flag was provided (after plugin discovery, before segmentation)
    if args.plugins {
        log::debug!("--plugins flag detected, listing all discovered plugins");
        let plugin_manager = crate::plugin::api::get_plugin_service().await;
        let plugins = plugin_manager.list_plugins_with_filter(false).await;

        if plugins.is_empty() {
            return Err(StartupError::ConfigurationError {
                message: "No plugins available after discovery".to_string(),
            });
        }

        display_plugin_table(plugins, use_color).map_err(|e| StartupError::DisplayFailed {
            message: e.to_string(),
        })?;

        // For --plugins flag, we return success with no scanner manager (indicating early exit)
        return Ok(None);
    }

    let segmenter = CommandSegmenter::with_commands(commands);
    let all_args: Vec<String> = std::env::args().collect();
    let command_segments = segmenter
        .segment_commands_only(&all_args, &args.global_args)
        .map_err(|e| {
            let error_msg = e.to_string();
            if error_msg.contains("Unexpected argument") && error_msg.contains("found after global")
            {
                // Extract the unknown command from the error message for better UX
                if let Some(start) = error_msg.find("'") {
                    if let Some(end) = error_msg.rfind("'") {
                        if start < end {
                            let unknown_cmd = &error_msg[start + 1..end];
                            return StartupError::CommandSegmentationFailed {
                                message: format!("Unknown command '{}'", unknown_cmd),
                            };
                        }
                    }
                }
            }

            // Fallback for other segmentation errors
            StartupError::CommandSegmentationFailed { message: error_msg }
        })?;

    // Stage 5: Plugin configuration (validate service dependencies first)
    log::debug!("Starting plugin configuration and service validation");
    configure_plugins(&command_segments, toml_config.as_ref()).await?;

    // Stage 6: Build query parameters from TOML config and CLI arguments
    let query_params = build_query_params(&final_args, toml_config.as_ref()).await?;

    // Stage 7: Scanner configuration and system integration
    let normalized_repositories = final_args.normalized_repositories();
    // Extract checkout settings from CLI arguments
    let checkout_settings = final_args.checkout_settings();

    // Extract case sensitivity override from CLI arguments
    let case_sensitivity_override = final_args.resolve_case_sensitivity_override();

    log::debug!(
        "Starting scanner configuration with {} repositories",
        normalized_repositories.len()
    );
    let scanner_manager_opt = configure_scanner(
        &normalized_repositories,
        query_params,
        checkout_settings,
        case_sensitivity_override,
    )
    .await;

    // Return the configured scanner manager for main.rs to use, or None if no valid scanners
    match scanner_manager_opt {
        Some(scanner_manager) => {
            log::debug!("System startup completed successfully");
            Ok(Some(scanner_manager))
        }
        None => {
            log::warn!("No valid scanners found during configuration. Startup aborted.");
            Ok(None)
        }
    }
}

/// Discover plugins and return list of available commands
async fn discover_commands(
    plugin_dir: Option<&str>,
    exclusions: &[String],
) -> Result<Vec<String>, PluginError> {
    log::trace!(
        "discover_commands starting with plugin_dir: {:?}, exclusions: {:?}",
        plugin_dir,
        exclusions
    );

    // Enhanced error context - use independent service access
    log::trace!("discover_commands getting plugin service independently");
    let mut plugin_manager = crate::plugin::api::get_plugin_service().await;
    log::trace!("discover_commands acquired plugin manager lock successfully");

    // Enhanced error handling with context
    log::trace!("discover_commands calling plugin_manager.discover_plugins");
    plugin_manager
        .discover_plugins(plugin_dir, exclusions)
        .await
        .map_err(|e| {
            log::warn!("Plugin discovery failed during plugin_manager.discover_plugins()");
            e
        })?;
    log::trace!("discover_commands plugin discovery completed successfully");

    let plugins = plugin_manager.list_plugins_with_filter(false).await;
    log::trace!(
        "discover_commands found {} plugins after listing",
        plugins.len()
    );

    // Validate that we found plugins
    if plugins.is_empty() {
        let error_msg = format!(
            "No plugins discovered in directory {:?} (exclusions: {:?})",
            plugin_dir, exclusions
        );
        log::error!("{}", error_msg);
        return Err(PluginError::LoadError {
            plugin_name: "plugin_discovery".to_string(),
            cause: error_msg,
        });
    }

    let command_names: Vec<String> = plugins
        .iter()
        .flat_map(|plugin| {
            plugin.functions.iter().flat_map(|func| {
                // Include both the primary function name and all aliases
                let mut names = vec![func.name.clone()];
                names.extend(func.aliases.clone());
                names
            })
        })
        .collect();

    // Validate that we have commands
    if command_names.is_empty() {
        let error_msg = format!(
            "Found {} plugins but no commands available (plugins may not implement required functions)",
            plugins.len()
        );
        log::error!("{}", error_msg);
        return Err(PluginError::LoadError {
            plugin_name: "plugin_discovery".to_string(),
            cause: error_msg,
        });
    }

    log::trace!(
        "discover_commands completed successfully, returning {} commands",
        command_names.len()
    );
    Ok(command_names)
}

/// Configure plugins based on command segments
async fn configure_plugins(
    command_segments: &[super::cli::command_segmenter::CommandSegment],
    toml_config: Option<&toml::Table>,
) -> StartupResult<()> {
    use log;

    // Enhanced validation with detailed context
    if command_segments.is_empty() {
        return Err(StartupError::ConfigurationError {
            message: "No processing plugins activated".to_string(),
        });
    }

    // Get plugin manager independently to avoid deadlocks
    let mut plugin_manager = crate::plugin::api::get_plugin_service().await;

    // Step 1: Set plugin configurations from TOML config if available
    if let Some(config) = toml_config {
        if let Err(e) = plugin_manager.set_plugin_configs(config) {
            return Err(StartupError::PluginFailed { error: e });
        }
    }

    // Step 2: Activate plugins based on command segments
    plugin_manager
        .activate_plugins(command_segments)
        .await
        .map_err(|e| StartupError::PluginFailed { error: e })?;

    // Step 3: Initialise active plugins with their configurations
    plugin_manager
        .initialize_active_plugins()
        .await
        .map_err(|e| StartupError::PluginFailed { error: e })?;

    // Step 4: Setup notification subscribers for plugins and plugin manager
    plugin_manager
        .setup_plugin_notification_subscribers()
        .await
        .map_err(|e| StartupError::PluginFailed { error: e })?;

    plugin_manager
        .setup_system_notification_subscriber()
        .await
        .map_err(|e| StartupError::PluginFailed { error: e })?;

    log::debug!("Plugins loaded successfully");
    Ok(())
}

/// Build query parameters from TOML config and CLI arguments with validation
/// TOML config values are applied first, then CLI arguments override them
async fn build_query_params(
    args: &super::cli::args::Args,
    toml_config: Option<&toml::Table>,
) -> StartupResult<crate::core::query::QueryParams> {
    use crate::core::date_parser;
    use crate::core::query::QueryParams;

    // Start with base query parameters
    let mut query_params = QueryParams::new();

    // Apply TOML config if available (these will be overridden by CLI args if specified)
    if let Some(config) = toml_config {
        // Apply filter configurations from TOML
        if let Some(filters) = config.get("filters").and_then(|v| v.as_table()) {
            // Date range from config
            let config_since = filters
                .get("since")
                .and_then(|v| v.as_str())
                .and_then(|s| date_parser::parse_date(s).ok());
            let config_until = filters
                .get("until")
                .and_then(|v| v.as_str())
                .and_then(|s| date_parser::parse_date(s).ok());

            query_params = query_params.with_date_range(config_since, config_until);

            // File patterns from config
            if let Some(files) = filters.get("files").and_then(|v| v.as_array()) {
                let files: Vec<String> = files
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                query_params = query_params.with_files(files);
            }

            if let Some(exclude_files) = filters.get("exclude_files").and_then(|v| v.as_array()) {
                let exclude_files: Vec<String> = exclude_files
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                query_params = query_params.with_exclude_files(exclude_files);
            }

            // Paths from config
            if let Some(paths) = filters.get("paths").and_then(|v| v.as_array()) {
                let paths: Vec<String> = paths
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                query_params = query_params.with_paths(paths);
            }

            if let Some(exclude_paths) = filters.get("exclude_paths").and_then(|v| v.as_array()) {
                let exclude_paths: Vec<String> = exclude_paths
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                query_params = query_params.with_exclude_paths(exclude_paths);
            }

            // Extensions from config
            if let Some(extensions) = filters.get("extensions").and_then(|v| v.as_array()) {
                let extensions: Vec<String> = extensions
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                query_params = query_params.with_extensions(extensions);
            }

            if let Some(exclude_extensions) =
                filters.get("exclude_extensions").and_then(|v| v.as_array())
            {
                let exclude_extensions: Vec<String> = exclude_extensions
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                query_params = query_params.with_exclude_extensions(exclude_extensions);
            }

            // Authors from config
            if let Some(authors) = filters.get("authors").and_then(|v| v.as_array()) {
                let authors: Vec<String> = authors
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                query_params = query_params.with_authors(authors);
            }

            if let Some(exclude_authors) = filters.get("exclude_authors").and_then(|v| v.as_array())
            {
                let exclude_authors: Vec<String> = exclude_authors
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                query_params = query_params.with_exclude_authors(exclude_authors);
            }

            // Other parameters from config
            if let Some(max_commits) = filters.get("max_commits").and_then(|v| v.as_integer()) {
                query_params = query_params.with_max_commits(Some(max_commits as usize));
            }

            if let Some(git_ref) = filters.get("ref").and_then(|v| v.as_str()) {
                query_params = query_params.with_git_ref(Some(git_ref.to_string()));
            }
        }
    }

    // Parse date range from CLI arguments (overrides config if specified)
    let since = match args.since.as_ref() {
        Some(since_str) => match date_parser::parse_date(since_str) {
            Ok(date) => Some(date),
            Err(e) => {
                return Err(StartupError::QueryValidationFailed {
                    message: format!("Invalid --since date '{}': {}", since_str, e),
                });
            }
        },
        None => None,
    };

    let until = match args.until.as_ref() {
        Some(until_str) => match date_parser::parse_date(until_str) {
            Ok(date) => Some(date),
            Err(e) => {
                return Err(StartupError::QueryValidationFailed {
                    message: format!("Invalid --until date '{}': {}", until_str, e),
                });
            }
        },
        None => None,
    };

    // Apply CLI arguments (these override TOML config)
    // Only apply if CLI args have values, otherwise keep TOML config values
    if since.is_some() || until.is_some() {
        query_params = query_params.with_date_range(since, until);
    }

    if !args.files.is_empty() {
        query_params = query_params.with_files(args.files.clone());
    }
    if !args.exclude_files.is_empty() {
        query_params = query_params.with_exclude_files(args.exclude_files.clone());
    }
    if !args.paths.is_empty() {
        query_params = query_params.with_paths(args.paths.clone());
    }
    if !args.exclude_paths.is_empty() {
        query_params = query_params.with_exclude_paths(args.exclude_paths.clone());
    }
    if !args.extensions.is_empty() {
        query_params = query_params.with_extensions(args.extensions.clone());
    }
    if !args.exclude_extensions.is_empty() {
        query_params = query_params.with_exclude_extensions(args.exclude_extensions.clone());
    }
    if !args.author.is_empty() {
        query_params = query_params.with_authors(args.author.clone());
    }
    if !args.exclude_author.is_empty() {
        query_params = query_params.with_exclude_authors(args.exclude_author.clone());
    }
    if args.max_commits.is_some() {
        query_params = query_params.with_max_commits(args.max_commits);
    }
    if args.git_ref.is_some() {
        query_params = query_params.with_git_ref(args.git_ref.clone());
    }

    // Validate query parameters
    if let Err(e) = query_params.validate() {
        return Err(StartupError::QueryValidationFailed {
            message: e.to_string(),
        });
    }

    Ok(query_params)
}

/// Configure scanner manager and integrate with plugins - returns configured ScannerManager
async fn configure_scanner(
    repositories: &[std::path::PathBuf],
    query_params: crate::core::query::QueryParams,
    checkout_settings: Option<crate::app::cli::CheckoutSettings>,
    case_sensitivity_override: Option<bool>,
) -> Option<std::sync::Arc<crate::scanner::api::ScannerManager>> {
    use crate::scanner::api::ScannerManager;

    // Repository list is already normalized upstream to include default current directory
    let repositories_to_scan = repositories.to_vec();

    // Step 1: Create ScannerManager with case sensitivity override
    let scanner_manager = if let Some(override_value) = case_sensitivity_override {
        std::sync::Arc::new(ScannerManager::with_case_sensitivity(Some(override_value)))
    } else {
        ScannerManager::create().await
    };

    // Step 2: Get plugin manager and check for active processing plugins
    let plugin_names = {
        let plugin_manager = crate::plugin::api::get_plugin_service().await;
        let active_plugins = plugin_manager.get_active_plugins();

        if active_plugins.is_empty() {
            log::error!("No active processing plugins found");
            return None;
        }

        // Extract plugin names before releasing the lock
        active_plugins
    }; // plugin_manager lock is released here

    // Step 3: Setup plugin integration
    let queue_manager = crate::queue::api::get_queue_service();
    {
        // Setup plugin consumers (get mutable access to plugin manager)
        let mut plugin_manager = crate::plugin::api::get_plugin_service().await;

        // Note: setup_plugin_consumers expects plugin_args, using empty for now
        let plugin_args: Vec<String> = Vec::new();

        if let Err(e) = plugin_manager
            .setup_plugin_consumers(&queue_manager, &plugin_names, &plugin_args)
            .await
        {
            log_error_with_context(&e, "Failed to setup plugin consumers for queue integration");
            log::debug!("Plugin names: {plugin_names:?}");
            return None;
        }
    }

    // Step 4: Create scanners for all repositories using batch method with all-or-nothing semantics
    match scanner_manager
        .create_scanners(
            &repositories_to_scan,
            Some(&query_params),
            checkout_settings.as_ref(),
        )
        .await
    {
        Ok(scanners) => {
            log::debug!(
                "Successfully created {} scanners for all repositories",
                scanners.len()
            );
        }
        Err(e) => {
            log::error!("Failed to initialise repository scan");
            log::debug!("Error: {e}");
            return None;
        }
    }

    // Return the configured scanner manager
    Some(scanner_manager)
}
