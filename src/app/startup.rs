use crate::core::services::get_services;
use crate::plugin::PluginError;
use log;
use std::process::exit;
use toml;

/// Core startup implementation - returns configured ScannerManager
///
/// # Error Handling
/// This function implements fail-fast error handling - any critical configuration
/// error will cause the process to exit with code 1. All resources are automatically
/// cleaned up by the OS on process exit, so no manual cleanup is required.
pub async fn startup(command_name: &str) -> Option<std::sync::Arc<crate::scanner::ScannerManager>> {
    use super::cli::args::Args;
    use super::cli::command_segmenter::CommandSegmenter;
    use super::cli::initial_args;
    use crate::core::logging::{init_logging, reconfigure_logging};
    use crate::core::strings::title_case;

    // Stage 1: Initial parsing for configuration discovery
    let args = initial_args(command_name);
    let command_title = title_case(command_name);
    let use_color =
        (args.color || std::io::IsTerminal::is_terminal(&std::io::stdout())) && !args.no_color;

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
        eprintln!("FATAL: Failed to initialize logging system");
        eprintln!("Error details: {e}");
        eprintln!("Startup cannot continue without functioning logging system");
        exit(1);
    } else {
        log::trace!("Initial args parsed and logging initialised");
    }

    // Stage 2: Command segmentation and config parsing
    log::trace!("Starting configuration file parsing");
    let mut final_args = Args::new();
    let toml_config =
        Args::parse_config_file_with_raw_config(&mut final_args, args.config_file.clone()).await;
    log::debug!(
        "Configuration file parsed successfully: config_file={:?}, has_toml_config={}",
        args.config_file,
        toml_config.is_some()
    );

    let plugin_dir = args
        .plugin_dir
        .clone()
        .or(final_args.plugin_dir.clone())
        .or(dirs::config_dir().map(|d| d.join(command_title).to_string_lossy().to_string()));
    let color = args.color || final_args.color;
    let no_color = args.no_color || final_args.no_color;
    let use_color = (color || std::io::IsTerminal::is_terminal(&std::io::stdout())) && !no_color;

    // Stage 2: Reconfigure logging with final values
    log::trace!("Reconfiguring logging with final values");
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
        eprintln!("FATAL: Failed to reconfigure logging system with final configuration");
        eprintln!("Error details: {e}");
        eprintln!(
            "Configuration: level={:?}, format={:?}, file={:?}, color={}",
            log_level, log_format, log_file, use_color
        );
        exit(1);
    }

    // Stage 3: Final global args parsing with collected arguments
    log::trace!("Starting final global args parsing");
    Args::parse_from_args(
        &mut final_args,
        command_name,
        &args.global_args,
        color,
        no_color,
    );
    log::debug!("Successfully parsed global args from collected arguments");
    log::trace!("Final args: {final_args:?}");

    // Stage 4: Command discovery and segmentation
    log::debug!("Starting command discovery phase");
    let _plugin_dir = plugin_dir.as_deref().or(final_args.plugin_dir.as_deref());
    log::trace!("Using plugin directory: {:?}", _plugin_dir);
    let commands = match discover_commands(_plugin_dir, &args.plugin_exclusions).await {
        Ok(commands) => {
            log::info!(
                "Successfully discovered {} commands: {:?}",
                commands.len(),
                commands
            );
            commands
        }
        Err(e) => {
            log::error!("FATAL: Plugin discovery failed - cannot proceed without plugins");
            log::error!("Error details: {e}");
            log::error!("Plugin directory: {:?}", _plugin_dir);
            log::error!("Exclusions: {:?}", args.plugin_exclusions);
            exit(1);
        }
    };

    log::debug!("Starting command line segmentation");
    let commands_clone = commands.clone();
    let segmenter = CommandSegmenter::with_commands(commands);
    let all_args: Vec<String> = std::env::args().collect();
    log::trace!("Command line arguments to segment: {:?}", all_args);
    let command_segments = segmenter
        .segment_commands_only(&all_args, &args.global_args)
        .unwrap_or_else(|e| {
            log::error!("FATAL: Command segmentation failed - cannot parse command line arguments");
            log::error!("Error details: {e}");
            log::error!("Available commands: {:?}", commands_clone);
            log::error!("Command line args: {:?}", all_args);
            log::error!("Global args: {:?}", args.global_args);
            exit(1);
        });

    log::info!(
        "Command segmentation completed successfully: {} command segments",
        command_segments.len()
    );
    log::trace!("Command segments: {:?}", command_segments);

    // Stage 5: Plugin configuration (validate service dependencies first)
    log::info!("Starting plugin configuration phase");
    log::trace!("Validating service startup dependencies before plugin configuration");
    validate_service_dependencies().await;
    configure_plugins(&command_segments, toml_config.as_ref()).await;

    // Stage 6: Build query parameters from TOML config and CLI arguments
    log::info!("Building query parameters from configuration and CLI arguments");
    let query_params = build_query_params(&final_args, toml_config.as_ref()).await;

    // Stage 7: Scanner configuration and system integration
    log::info!("Starting scanner configuration and system integration");
    let scanner_manager_opt = configure_scanner(&final_args.repository, query_params).await;

    // Return the configured scanner manager for main.rs to use, or None if no valid scanners
    match scanner_manager_opt {
        Some(scanner_manager) => {
            log::info!("✅ System startup completed successfully - all components ready");
            log::debug!("Returning configured scanner manager to main process");
            Some(scanner_manager)
        }
        None => {
            log::warn!("No valid scanners found during configuration. Startup aborted.");
            None
        }
    }
}

/// Discover plugins and return list of available commands
async fn discover_commands(
    plugin_dir: Option<&str>,
    exclusions: &[String],
) -> Result<Vec<String>, PluginError> {
    log::trace!("Plugin discovery - dir: {plugin_dir:?}");

    // Enhanced error context - capture initial state
    let services = get_services();
    let mut plugin_manager = services.plugin_manager().await;

    log::debug!(
        "Starting plugin discovery with dir: {plugin_dir:?}, exclusions: {:?}",
        exclusions
    );

    // Enhanced error handling with context
    plugin_manager
        .discover_plugins(plugin_dir, exclusions)
        .await
        .map_err(|e| {
            log::error!("Plugin discovery failed during plugin_manager.discover_plugins()");
            log::error!("Plugin directory: {:?}", plugin_dir);
            log::error!("Exclusions: {:?}", exclusions);
            e
        })?;

    let plugins = plugin_manager.list_plugins_with_filter(false).await;

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
        .flat_map(|plugin| plugin.functions.iter().map(|func| func.name.clone()))
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

    log::debug!(
        "Discovered {} plugins with {} commands: {:?}",
        plugins.len(),
        command_names.len(),
        command_names
    );

    Ok(command_names)
}

/// Configure plugins based on command segments
async fn configure_plugins(
    command_segments: &[super::cli::command_segmenter::CommandSegment],
    toml_config: Option<&toml::Table>,
) {
    use log::{error, info};

    log::trace!(
        "Configuring plugins for {} command segments",
        command_segments.len()
    );

    // Enhanced validation with detailed context
    if command_segments.is_empty() {
        error!("FATAL: No processing plugins activated - unable to proceed with scanning");
        error!("This typically indicates:");
        error!("  1. No valid commands were provided on the command line");
        error!("  2. All provided commands were excluded by plugin exclusions");
        error!("  3. Plugin discovery found no matching plugins for the commands");
        error!("Available plugins should have been discovered in previous step");
        exit(1);
    }

    // Get plugin manager from services
    let services = get_services();
    let mut plugin_manager = services.plugin_manager().await;

    // Step 1: Set plugin configurations from TOML config if available
    if let Some(config) = toml_config {
        if let Err(e) = plugin_manager.set_plugin_configs(config) {
            error!("FATAL: Failed to set plugin configurations from TOML config");
            error!("Error details: {}", e);
            error!(
                "Configuration keys: {:?}",
                config.keys().collect::<Vec<_>>()
            );
            error!("This indicates the TOML configuration contains invalid plugin settings");
            exit(1);
        } else {
            info!("TOML plugin configurations applied successfully");
        }
    } else {
        info!("No TOML configuration found, plugins will use default settings");
    }

    // Step 2: Activate plugins based on command segments
    if let Err(e) = plugin_manager.activate_plugins(command_segments).await {
        error!("FATAL: Failed to activate plugins based on command segments");
        error!("Error details: {}", e);
        error!(
            "Command segments: {:?}",
            command_segments
                .iter()
                .map(|seg| &seg.command_name)
                .collect::<Vec<_>>()
        );
        error!("This indicates plugin activation failed during startup");
        exit(1);
    }

    // Step 3: Initialize active plugins with their configurations
    if let Err(e) = plugin_manager.initialize_active_plugins().await {
        error!("FATAL: Failed to initialize active plugins");
        error!("Error details: {}", e);
        error!("This indicates one or more plugins failed during initialization");
        error!("Check plugin-specific logs for more detailed error information");
        exit(1);
    }

    // Step 4: Setup notification subscribers for plugins and plugin manager
    if let Err(e) = plugin_manager.setup_plugin_notification_subscribers().await {
        error!("FATAL: Failed to setup plugin notification subscribers");
        error!("Error details: {}", e);
        error!("This indicates the notification system integration failed");
        error!("Plugins may not receive system events properly");
        exit(1);
    }

    if let Err(e) = plugin_manager.setup_system_notification_subscriber().await {
        error!("FATAL: Failed to setup plugin manager system notification subscriber");
        error!("Error details: {}", e);
        error!("This indicates the plugin manager cannot receive system events");
        error!("Plugin lifecycle management may be impaired");
        exit(1);
    }

    info!("Plugin configuration completed successfully (including notification subscribers)");
}

/// Build query parameters from TOML config and CLI arguments with validation
/// TOML config values are applied first, then CLI arguments override them
async fn build_query_params(
    args: &super::cli::args::Args,
    toml_config: Option<&toml::Table>,
) -> crate::core::query::QueryParams {
    use crate::app::cli::date_parser;
    use crate::core::query::QueryParams;
    use log::{debug, error, trace};
    use std::process::exit;

    trace!("Building query parameters from TOML config and CLI arguments");

    // Start with base query parameters
    let mut query_params = QueryParams::new();

    // Apply TOML config if available (these will be overridden by CLI args if specified)
    if let Some(config) = toml_config {
        trace!("Applying TOML configuration to query parameters");

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

        debug!("TOML configuration applied to query parameters");
    }

    // Parse date range from CLI arguments (overrides config if specified)
    let since = match args.since.as_ref() {
        Some(since_str) => match date_parser::parse_date(since_str) {
            Ok(date) => {
                debug!("Parsed CLI since date: {} -> {:?}", since_str, date);
                Some(date)
            }
            Err(e) => {
                error!("Invalid --since date '{}': {}", since_str, e);
                exit(1);
            }
        },
        None => None,
    };

    let until = match args.until.as_ref() {
        Some(until_str) => match date_parser::parse_date(until_str) {
            Ok(date) => {
                debug!("Parsed CLI until date: {} -> {:?}", until_str, date);
                Some(date)
            }
            Err(e) => {
                error!("Invalid --until date '{}': {}", until_str, e);
                exit(1);
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
        error!("Invalid query parameters: {}", e);
        exit(1);
    }

    debug!("Query parameters built and validated successfully");
    query_params
}

/// Configure scanner manager and integrate with plugins - returns configured ScannerManager
async fn configure_scanner(
    repositories: &[std::path::PathBuf],
    query_params: crate::core::query::QueryParams,
) -> Option<std::sync::Arc<crate::scanner::ScannerManager>> {
    use crate::scanner::ScannerManager;
    use log::{debug, error, trace, warn};

    trace!("Starting scanner configuration");

    // Default to current directory if no repositories specified
    let repositories_to_scan = if repositories.is_empty() {
        debug!("No repositories specified, defaulting to current directory");
        vec![std::path::PathBuf::from(".")]
    } else {
        repositories.to_vec()
    };

    // Step 1: Create ScannerManager
    let scanner_manager = ScannerManager::create().await;
    debug!("Scanner manager created successfully");

    // Step 2: Get plugin manager and check for active processing plugins
    let services = get_services();
    let plugin_names = {
        let plugin_manager = services.plugin_manager().await;
        let active_plugins = plugin_manager.get_active_plugins();

        if active_plugins.is_empty() {
            error!("No active processing plugins found");
            error!("Scanner requires at least one active plugin to process scan results");
            error!("This indicates plugin activation in previous step failed silently");
            error!("Check plugin configuration and activation logs above");
            return None;
        }

        trace!(
            "Found {} active plugins for processing",
            active_plugins.len()
        );

        // Extract plugin names before releasing the lock
        active_plugins
            .iter()
            .map(|info| info.plugin_name.clone())
            .collect::<Vec<String>>()
    }; // plugin_manager lock is released here

    // Step 3: Setup plugin integration
    let queue_manager = services.queue_manager();
    {
        // Setup plugin consumers (get mutable access to plugin manager)
        let mut plugin_manager = services.plugin_manager().await;

        // Note: setup_plugin_consumers expects plugin_args, using empty for now
        let plugin_args: Vec<String> = Vec::new();

        if let Err(e) = plugin_manager
            .setup_plugin_consumers(&queue_manager, &plugin_names, &plugin_args)
            .await
        {
            error!("Failed to setup plugin consumers for queue integration");
            error!("Error details: {}", e);
            error!("Plugin names: {:?}", plugin_names);
            error!("This indicates plugins cannot receive scan messages from the queue");
            error!("Scanner and plugin integration has failed");
            return None;
        }

        debug!(
            "Plugin consumers setup completed for {} plugins",
            plugin_names.len()
        );
    }

    // Step 4: Create scanners for all repositories
    let mut successful_scanners = 0;
    let mut failed_repositories = Vec::new();

    for (index, repo_path) in repositories_to_scan.iter().enumerate() {
        let repo_path_str = repo_path.to_string_lossy();

        if let Err(e) = scanner_manager.create_scanner(&repo_path_str).await {
            error!(
                "Failed to create scanner for repository '{}' (#{}/{})",
                repo_path_str,
                index + 1,
                repositories_to_scan.len()
            );
            error!("Error details: {}", e);
            error!("Repository path: {}", repo_path_str);
            error!("This typically indicates:");
            error!("  1. Repository path does not exist or is not accessible");
            error!("  2. Repository is not a valid Git repository");
            error!("  3. Permissions issue accessing the repository");
            failed_repositories.push(repo_path_str.to_string());
        } else {
            successful_scanners += 1;
            debug!(
                "Scanner created successfully for repository: {} (#{}/{})",
                repo_path_str,
                index + 1,
                repositories_to_scan.len()
            );
        }
    }

    // Check if we have any successful scanners
    if successful_scanners == 0 {
        error!("Failed to create scanners for any repositories");
        error!(
            "Total repositories attempted: {}",
            repositories_to_scan.len()
        );
        error!("Failed repositories: {:?}", failed_repositories);
        error!("No valid repositories available for scanning");
        return None;
    } else if !failed_repositories.is_empty() {
        warn!(
            "Some repositories failed to initialize: {} successful, {} failed",
            successful_scanners,
            failed_repositories.len()
        );
        warn!("Failed repositories: {:?}", failed_repositories);
        warn!("Continuing with {} valid repositories", successful_scanners);
    }

    debug!("Query parameters configured: {:?}", query_params);
    debug!(
        "Scanner configuration completed successfully for {} repositories",
        successful_scanners
    );

    // Step 5: Publish system startup event to activate plugin consumers
    if let Err(e) = publish_system_startup_event().await {
        error!("Failed to publish system startup event: {}", e);
        error!("Error details: {}", e);
        error!("This indicates the notification system failed to broadcast system readiness");
        // This is not necessarily fatal as the system is otherwise configured
        warn!("Plugin consumers may not be activated automatically");
        warn!("Manual plugin activation may be required");
        warn!("Scanner functionality may be impaired but system can continue");
    } else {
        debug!("System startup event published successfully - plugins should be activated");
    }

    // Return the configured scanner manager
    Some(scanner_manager)
}

/// Validate that all required services are available and properly initialized
/// This ensures service startup dependencies are met before proceeding with configuration
async fn validate_service_dependencies() {
    use log::{debug, error, trace};
    use std::process::exit;

    trace!("Starting service dependency validation");

    let services = get_services();

    // Validate PluginManager service
    {
        let plugin_manager = services.plugin_manager().await;
        debug!("✓ PluginManager service is available and accessible");

        // Test basic plugin manager functionality
        let plugin_count = plugin_manager.list_plugins_with_filter(false).await.len();
        debug!("✓ PluginManager reports {} plugins available", plugin_count);
    }

    // Validate QueueManager service
    {
        let queue_manager = services.queue_manager();
        debug!("✓ QueueManager service is available and accessible");

        // Test basic queue manager functionality by creating a test consumer
        match queue_manager.create_consumer("startup_dependency_test".to_string()) {
            Ok(_) => {
                debug!("✓ QueueManager can create consumers successfully");
            }
            Err(e) => {
                error!("FATAL: QueueManager service validation failed");
                error!("Cannot create test consumer: {}", e);
                error!("Queue system is not functioning properly");
                exit(1);
            }
        }
    }

    // Validate NotificationManager service
    {
        let _notification_manager = services.notification_manager().await;
        debug!("✓ NotificationManager service is available and accessible");

        // Note: We don't test notification publishing here to avoid side effects
        // The service availability check is sufficient for dependency validation
    }

    debug!("✓ All service dependencies validated successfully");
    trace!("Service dependency validation completed");
}

/// Publish system startup event to notify all components that system is ready
async fn publish_system_startup_event() -> Result<(), Box<dyn std::error::Error>> {
    use crate::notifications::event::{Event, SystemEvent, SystemEventType};
    use log::{debug, info};

    debug!("Publishing system startup event");

    let services = get_services();
    let mut notification_manager = services.notification_manager().await;

    let startup_event = Event::System(SystemEvent::with_message(
        SystemEventType::Startup,
        "System initialization completed - plugins may now activate".to_string(),
    ));

    notification_manager.publish(startup_event).await?;

    info!("System startup event published successfully");
    Ok(())
}
