use log;

/// Core startup implementation
pub async fn startup(command_name: &str) {
    use super::cli::args::Args;
    use super::cli::command_segmenter::CommandSegmenter;
    use super::cli::{initial_args, RequiredArgs};
    use crate::core::logging::{init_logging, reconfigure_logging};
    use crate::core::strings::title_case;

    // Stage 1: Initial parsing for configuration discovery
    let RequiredArgs {
        global_args,
        config_file,
        plugin_dir,
        color,
        no_color,
        log_format,
        log_level,
        log_file,
    } = initial_args(command_name);
    let command_title = title_case(command_name);
    let use_color = (color || atty::is(atty::Stream::Stdout)) && !no_color;

    // Initialize logging
    let log_file_str = log_file.as_ref().map(|p| p.to_string_lossy().to_string());
    if let Err(e) = init_logging(
        log_level.as_deref(),
        log_format.as_deref(),
        log_file_str.as_deref(),
        use_color,
    ) {
        eprintln!("Failed to initialize logging: {e}");
    } else {
        log::trace!("Initial args parsed and logging initialised");
    }

    // Stage 1: Command segmentation
    let mut final_args = Args::new();
    Args::parse_config_file_async(&mut final_args, config_file).await;
    log::trace!("Configuration file parsed");

    let plugin_dir = plugin_dir
        .clone()
        .or(final_args.plugin_dir.clone())
        .or(dirs::config_dir().map(|d| d.join(command_title).to_string_lossy().to_string()));
    let color = color || final_args.color;
    let no_color = no_color || final_args.no_color;
    let use_color = (color || atty::is(atty::Stream::Stdout)) && !no_color;

    // Stage 2: Reconfigure logging with final values
    log::trace!("Reconfiguring logging with final values");
    let log_level = log_level.clone().or(final_args.log_level.clone());
    let log_format = log_format.clone().or(final_args.log_format.clone());
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
        eprintln!("Failed to reconfigure logging: {e}");
    }

    // Stage 3: Final global args parsing with collected arguments
    Args::parse_from_args(&mut final_args, command_name, &global_args, color, no_color);
    log::trace!(
        "Successfully parsed global args from collected arguments: {:?}",
        global_args
    );

    // Stage 4: Command discovery and segmentation
    let _plugin_dir = plugin_dir.as_deref().or(final_args.plugin_dir.as_deref());
    let commands = discover_commands(_plugin_dir);
    log::trace!("Discovered commands: {:?}", commands);

    let segmenter = CommandSegmenter::with_commands(commands);
    let all_args: Vec<String> = std::env::args().collect();
    let command_segments = segmenter
        .segment_commands_only(&all_args, &global_args)
        .unwrap_or_else(|e| {
            log::error!("Failed to segment commands: {e}");
            std::process::exit(1);
        });

    // Stage 5: Plugin configuration
    configure_plugins(&command_segments);
}

/// Discover plugins and return list of available commands
fn discover_commands(plugin_dir: Option<&str>) -> Vec<String> {
    log::trace!("Plugin discovery - dir: {plugin_dir:?}");

    vec![
        "dump".to_string(),
        "commits".to_string(),
        "metrics".to_string(),
        "export".to_string(),
    ]
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
