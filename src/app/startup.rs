use crate::app::cli::display::display_plugin_table;
use crate::core::services::get_services;
use crate::plugin::api::PluginError;
use log;
use std::process::exit;
use toml;

/// Core startup implementation - returns configured ScannerManager
///
/// # Error Handling
/// This function implements fail-fast error handling - any critical configuration
/// error will cause the process to exit with code 1. All resources are automatically
/// cleaned up by the OS on process exit, so no manual cleanup is required.
pub async fn startup(
    command_name: &str,
) -> Option<std::sync::Arc<crate::scanner::api::ScannerManager>> {
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
        eprintln!("FATAL: Failed to initialize logging: {e}");
        exit(1);
    } else {
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
    let color = args.color || final_args.color;
    let no_color = args.no_color || final_args.no_color;
    let use_color = (color || std::io::IsTerminal::is_terminal(&std::io::stdout())) && !no_color;

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
        eprintln!("FATAL: Failed to reconfigure logging system with final configuration");
        eprintln!("Error details: {e}");
        eprintln!(
            "Configuration: level={:?}, format={:?}, file={:?}, color={}",
            log_level, log_format, log_file, use_color
        );
        exit(1);
    }

    // Stage 3: Final global args parsing with collected arguments
    Args::parse_from_args(
        &mut final_args,
        command_name,
        &args.global_args,
        color,
        no_color,
    );

    // Stage 4: Command discovery and segmentation
    let _plugin_dir = plugin_dir.as_deref().or(final_args.plugin_dir.as_deref());
    let commands = match discover_commands(_plugin_dir, &args.plugin_exclusions).await {
        Ok(commands) => commands,
        Err(e) => {
            log::error!("FATAL: Plugin discovery failed - cannot proceed without plugins: {e}");
            exit(1);
        }
    };

    // Check if --plugins flag was provided (after plugin discovery, before segmentation)
    if args.plugins {
        log::debug!("--plugins flag detected, listing all discovered plugins");
        let services = get_services();
        let plugin_manager = services.plugin_manager().await;
        let plugins = plugin_manager.list_plugins_with_filter(false).await;

        if plugins.is_empty() {
            log::error!("No plugins available after discovery");
            exit(1);
        }

        if let Err(e) = display_plugin_table(plugins, use_color) {
            log::error!("Failed to display plugin table: {}", e);
            exit(1);
        }
        exit(0);
    }

    let segmenter = CommandSegmenter::with_commands(commands);
    let all_args: Vec<String> = std::env::args().collect();
    let command_segments = segmenter
        .segment_commands_only(&all_args, &args.global_args)
        .unwrap_or_else(|e| {
            let error_msg = e.to_string();
            if error_msg.contains("Unexpected argument") && error_msg.contains("found after global")
            {
                // Extract the unknown command from the error message
                if let Some(start) = error_msg.find("'") {
                    if let Some(end) = error_msg.rfind("'") {
                        if start < end {
                            let unknown_cmd = &error_msg[start + 1..end];
                            log::error!("Unknown command '{}'", unknown_cmd);
                            exit(1);
                        }
                    }
                }
            }

            // Fallback for other segmentation errors
            log::error!("FATAL: Command segmentation failed: {e}");
            exit(1);
        });

    // Stage 5: Plugin configuration (validate service dependencies first)
    log::debug!("Starting plugin configuration and service validation");
    configure_plugins(&command_segments, toml_config.as_ref()).await;

    // Stage 6: Build query parameters from TOML config and CLI arguments
    let query_params = build_query_params(&final_args, toml_config.as_ref()).await;

    // Stage 7: Scanner configuration and system integration
    log::debug!(
        "Starting scanner configuration with {} repositories",
        final_args.repository.len()
    );
    let scanner_manager_opt = configure_scanner(&final_args.repository, query_params).await;

    // Return the configured scanner manager for main.rs to use, or None if no valid scanners
    match scanner_manager_opt {
        Some(scanner_manager) => {
            log::debug!("System startup completed successfully");
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
    // Enhanced error context - capture initial state
    let services = get_services();
    let mut plugin_manager = services.plugin_manager().await;

    // Enhanced error handling with context
    plugin_manager
        .discover_plugins(plugin_dir, exclusions)
        .await
        .map_err(|e| {
            log::warn!("Plugin discovery failed during plugin_manager.discover_plugins()");
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

    Ok(command_names)
}

/// Configure plugins based on command segments
async fn configure_plugins(
    command_segments: &[super::cli::command_segmenter::CommandSegment],
    toml_config: Option<&toml::Table>,
) {
    use log;

    // Enhanced validation with detailed context
    if command_segments.is_empty() {
        log::error!("FATAL: No processing plugins activated");
        exit(1);
    }

    // Get plugin manager from services
    let services = get_services();
    let mut plugin_manager = services.plugin_manager().await;

    // Step 1: Set plugin configurations from TOML config if available
    if let Some(config) = toml_config {
        if let Err(e) = plugin_manager.set_plugin_configs(config) {
            log::error!("FATAL: Failed to set plugin configurations from TOML config");
            log::debug!("Error details: {}", e);
            exit(1);
        }
    }

    // Step 2: Activate plugins based on command segments
    if let Err(e) = plugin_manager.activate_plugins(command_segments).await {
        log::error!("FATAL: Could not activate plugins");
        log::debug!("Error details: {}", e);
        exit(1);
    }

    // Step 3: Initialise active plugins with their configurations
    if let Err(e) = plugin_manager.initialize_active_plugins().await {
        log::error!("FATAL: Plugin initialization failed");
        log::debug!("Error details: {}", e);
        exit(1);
    }

    // Step 4: Setup notification subscribers for plugins and plugin manager
    if let Err(e) = plugin_manager.setup_plugin_notification_subscribers().await {
        log::error!("FATAL: Failed to setup plugins");
        log::debug!("Error details: {}", e);
        exit(1);
    }

    if let Err(e) = plugin_manager.setup_system_notification_subscriber().await {
        log::error!("FATAL: Failed to setup plugin manager");
        log::debug!("Error details: {}", e);
        exit(1);
    }

    log::debug!("Plugins loaded successfully");
}

/// Build query parameters from TOML config and CLI arguments with validation
/// TOML config values are applied first, then CLI arguments override them
async fn build_query_params(
    args: &super::cli::args::Args,
    toml_config: Option<&toml::Table>,
) -> crate::core::query::QueryParams {
    use crate::app::cli::date_parser;
    use crate::core::query::QueryParams;
    use std::process::exit;

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
                log::error!("Invalid --since date '{}': {}", since_str, e);
                exit(1);
            }
        },
        None => None,
    };

    let until = match args.until.as_ref() {
        Some(until_str) => match date_parser::parse_date(until_str) {
            Ok(date) => Some(date),
            Err(e) => {
                log::error!("Invalid --until date '{}': {}", until_str, e);
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
        log::error!("Invalid query parameters: {}", e);
        exit(1);
    }

    query_params
}

/// Configure scanner manager and integrate with plugins - returns configured ScannerManager
async fn configure_scanner(
    repositories: &[std::path::PathBuf],
    query_params: crate::core::query::QueryParams,
) -> Option<std::sync::Arc<crate::scanner::api::ScannerManager>> {
    use crate::scanner::api::ScannerManager;

    // Default to current directory if no repositories specified
    let repositories_to_scan = if repositories.is_empty() {
        vec![std::path::PathBuf::from(".")]
    } else {
        repositories.to_vec()
    };

    // Step 1: Create ScannerManager
    let scanner_manager = ScannerManager::create().await;

    // Step 2: Get plugin manager and check for active processing plugins
    let services = get_services();
    let plugin_names = {
        let plugin_manager = services.plugin_manager().await;
        let active_plugins = plugin_manager.get_active_plugins();

        if active_plugins.is_empty() {
            log::error!("No active processing plugins found");
            return None;
        }

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
            log::error!("Failed to setup plugin consumers for queue integration");
            log::debug!("Error details: {e} - Plugin names: {plugin_names:?}");
            return None;
        }
    }

    // Step 4: Create scanners for all repositories using batch method with all-or-nothing semantics
    match scanner_manager
        .create_scanners(&repositories_to_scan, Some(&query_params))
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
