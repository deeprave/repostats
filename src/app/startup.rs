use log;

/// Initialize application startup (async version)
pub async fn startup_async() {
    startup_impl().await;
}

/// Core startup implementation - now async
async fn startup_impl() {
    use super::cli::command_segmenter::CommandSegmenter;
    use super::cli::global_args::Args;
    use super::cli::initial_args;
    use super::cli::InitialArgsBundle;
    use crate::core::logging::{init_logging, reconfigure_logging};
    use crate::core::strings::title_case;

    // Stage 1: Initial parsing for configuration discovery
    let InitialArgsBundle {
        command_name,
        config_file,
        plugin_dir,
        plugin_exclude,
        color,
        no_color,
        log_format,
        log_level,
        log_file,
    } = initial_args();
    let command_title = title_case(command_name.as_str());
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
    let plugin_exclude = plugin_exclude.clone().or(final_args.plugin_exclude.clone());
    let color = color || final_args.color;
    let no_color = no_color || final_args.no_color;
    let use_color = (color || atty::is(atty::Stream::Stdout)) && !no_color;

    // Stage 2: Command segmentation
    log::trace!("Starting processing functions");
    let commands = discover_commands(plugin_dir.as_deref(), plugin_exclude.as_deref());
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

    let segmenter = CommandSegmenter::with_commands(commands);
    let args: Vec<String> = std::env::args().collect();

    match segmenter.segment_arguments(&args) {
        Ok(segmented) => {
            log::trace!(
                "Segmentation success - Global: {:?}, Commands: {:?}",
                segmented.global_args,
                segmented.command_segments
            );

            // Stage 3: Final global args parsing with clean arguments
            Args::parse_from_args(&mut final_args, &segmented.global_args, color, no_color);
            log::trace!("Successfully parsed global args");

            // TODO: Continue with main application logic
            log::info!("{command_name}: Repository Statistics Tool starting");
            println!("=== Final Parsed Arguments ===");
            println!("{final_args:#?}");
        }
        Err(e) => {
            log::error!("Error segmenting arguments: {e}");
            std::process::exit(1);
        }
    }
}

/// Discover plugins and return list of available commands
fn discover_commands(plugin_dir: Option<&str>, plugin_exclude: Option<&str>) -> Vec<String> {
    log::trace!("Plugin discovery - dir: {plugin_dir:?}, exclude: {plugin_exclude:?}");

    vec![
        "debug".to_string(),
        "commits".to_string(),
        "metrics".to_string(),
        "export".to_string(),
    ]
}
