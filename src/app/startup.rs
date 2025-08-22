use crate::common::logging::set_logging_level;

/// Initialize application startup
pub fn startup() {
    use super::cli::global_args::Args;
    use super::cli::initial_args;
    use super::cli::command_segmenter::CommandSegmenter;
    use crate::common::logging::{init_logging, configure_logging};
    use crate::common::strings::title_case;

    // Stage 1: Initial parsing for configuration discovery
    let (command_name,
        config_file,
        plugin_dir,
        plugin_exclude,
        verbosity,
        color,
        no_color,
        log_format,
        log_level,
        log_file) = initial_args();
    let command_title = title_case(command_name.as_str());
    let use_color = (color || atty::is(atty::Stream::Stdout)) && !no_color;

    // Initialize logging
    init_logging(log_level.as_deref(), log_format.as_deref(), log_file.as_deref(), use_color);
    set_logging_level(verbosity);

    tracing::info!("{}: Repository Statistics Tool starting", command_name);

    // First, load the configuration file
    let mut final_args = Args::new();
    Args::parse_config_file(&mut final_args, config_file);

    let plugin_dir = plugin_dir.clone()
                                        .or(final_args.plugin_dir.clone())
                                        .or(dirs::config_dir().map(|d| d.join(command_title).to_string_lossy().to_string()));
    let plugin_exclude = plugin_exclude.clone().or(final_args.plugin_exclude.clone());
    let color = color || final_args.color;
    let no_color = no_color || final_args.no_color;
    let use_color = (color || atty::is(atty::Stream::Stdout)) && !no_color;
    let verbosity = (final_args.verbose as i8) - (final_args.quiet as i8);
    set_logging_level(verbosity);

    // Stage 2: Command segmentation
    tracing::debug!("Starting command segmentation");
    let commands = discover_commands(plugin_dir.as_deref(), plugin_exclude.as_deref());

    let segmenter = CommandSegmenter::with_commands(commands);
    let args: Vec<String> = std::env::args().collect();

    match segmenter.segment_arguments(&args) {
        Ok(segmented) => {
            tracing::debug!("Segmentation results - Global args: {:?}, Command segments: {:?}",
                           segmented.global_args, segmented.command_segments);

            // Stage 3: Final global args parsing with clean arguments
            tracing::debug!("Parsing final global arguments");
            Args::parse_from_args(&mut final_args, &segmented.global_args, color, no_color);

            configure_logging(final_args.log_level.as_deref(), final_args.log_format.as_deref(), final_args.log_file.as_deref(), use_color);

            tracing::debug!("Final arguments: {:#?}", final_args);

            // TODO: Continue with main application logic
            println!("=== Final Parsed Arguments ===");
            println!("{:#?}", final_args);
        },
        Err(e) => {
            tracing::error!("Error segmenting arguments: {}", e);
            std::process::exit(1);
        }
    }
}


/// Discover plugins and return list of available commands
fn discover_commands(plugin_dir: Option<&str>, plugin_exclude: Option<&str>) -> Vec<String> {

    tracing::debug!("Plugin discovery - dir: {:?}, exclude: {:?}", plugin_dir, plugin_exclude);
    
    vec![
        "debug".to_string(),
        "commits".to_string(),
        "metrics".to_string(),
        "export".to_string(),
    ]
}