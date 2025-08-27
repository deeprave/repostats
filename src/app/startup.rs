use std::process::exit;
use log;
use crate::core::services::get_services;
use crate::plugin::PluginError;

/// Core startup implementation
pub async fn startup(command_name: &str) {
    use super::cli::args::Args;
    use super::cli::command_segmenter::CommandSegmenter;
    use super::cli::initial_args;
    use crate::core::logging::{init_logging, reconfigure_logging};
    use crate::core::strings::title_case;

    // Stage 1: Initial parsing for configuration discovery
    let args = initial_args(command_name);
    let command_title = title_case(command_name);
    let use_color = (args.color || std::io::IsTerminal::is_terminal(&std::io::stdout())) && !args.no_color;

    // 1.1 Initialize logging
    let log_file_str = args.log_file.as_ref().map(|p| p.to_string_lossy().to_string());
    if let Err(e) = init_logging(args.log_level.as_deref(), args.log_format.as_deref(), log_file_str.as_deref(), use_color) {
        eprintln!("Failed to initialize logging: {e}");
        exit(1);
    } else {
        log::trace!("Initial args parsed and logging initialised");
    }

    // Stage 2: Command segmentation
    let mut final_args = Args::new();
    Args::parse_config_file_async(&mut final_args, args.config_file).await;
    log::trace!("Configuration file parsed");

    let plugin_dir = args.plugin_dir
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
    let log_file = log_file_str.clone().or(final_args.log_file.as_ref().map(|p| p.to_string_lossy().to_string()));
    if let Err(e) = reconfigure_logging(log_level.as_deref(), log_format.as_deref(), log_file.as_deref(), use_color) {
        eprintln!("Failed to reconfigure logging: {e}");
        exit(1);
    }

    // Stage 3: Final global args parsing with collected arguments
    Args::parse_from_args(&mut final_args, command_name, &args.global_args, color, no_color);
    log::trace!("Successfully parsed global args from collected arguments: {final_args:?}");

    // Stage 4: Command discovery and segmentation
    let _plugin_dir = plugin_dir.as_deref().or(final_args.plugin_dir.as_deref());
    let commands = match discover_commands(_plugin_dir, &args.plugin_exclusions).await {
        Ok(commands) => {
            log::trace!("Discovered commands: {:?}", commands);
            commands
        },
        Err(e) => {
            log::error!("Failed to discover plugins: {e}");
            exit(1);
        }
    };

    let segmenter = CommandSegmenter::with_commands(commands);
    let all_args: Vec<String> = std::env::args().collect();
    let command_segments = segmenter
        .segment_commands_only(&all_args, &args.global_args)
        .unwrap_or_else(|e| {
            log::error!("Failed to segment commands: {e}");
            exit(1);
        });

    // Stage 5: Plugin configuration
    configure_plugins(&command_segments);
}

/// Discover plugins and return list of available commands
async fn discover_commands(plugin_dir: Option<&str>, exclusions: &[String]) -> Result<Vec<String>, PluginError> {
    log::trace!("Plugin discovery - dir: {plugin_dir:?}");

    let services = get_services();
    let mut plugin_manager = services.plugin_manager().await;

    log::debug!("Starting plugin discovery with dir: {plugin_dir:?}");

    plugin_manager.discover_plugins(plugin_dir, exclusions).await?;

    let plugins = plugin_manager.list_plugins_with_filter(false).await;
    let command_names: Vec<String> = plugins
        .iter()
        .flat_map(|plugin| plugin.functions.iter().map(|func| func.name.clone()))
        .collect();

    log::info!("Discovered {} plugins with {} commands: {:?}", 
               plugins.len(), command_names.len(), command_names);

    Ok(command_names)
}

/// Configure plugins based on command segments
fn configure_plugins(_command_segments: &[super::cli::command_segmenter::CommandSegment]) {
    log::trace!(
        "Configuring plugins for {} command segments",
        _command_segments.len()
    );

    // TODO: Implement plugin configuration once plugin manager is available
    // This will process each command segment and configure the appropriate plugins
}
