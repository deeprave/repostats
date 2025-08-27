//! CLI module containing argument parsing and related functionality

use std::path::PathBuf;

pub mod api;
pub mod args;
pub mod command_segmenter;
pub mod date_parser;

pub struct RequiredArgs {
    pub global_args: Vec<String>,
    pub config_file: Option<PathBuf>,
    pub plugin_dir: Option<String>,
    pub plugin_exclusions: Vec<String>,
    pub color: bool,
    pub no_color: bool,
    pub log_format: Option<String>,
    pub log_level: Option<String>,
    pub log_file: Option<PathBuf>,
}

pub fn initial_args(command_name: &str) -> RequiredArgs {
    use args::Args;
    use std::env;

    // Get the command line arguments
    let cli_args: Vec<String> = env::args().collect();

    // Parse using Args in initial mode
    let (parsed_args, global_args) = Args::parse_initial(command_name, &cli_args);

    RequiredArgs {
        global_args,
        config_file: parsed_args.config_file,
        plugin_dir: parsed_args.plugin_dir,
        plugin_exclusions: parsed_args.plugin_exclusions,
        color: parsed_args.color,
        no_color: parsed_args.no_color,
        log_format: parsed_args.log_format,
        log_level: parsed_args.log_level,
        log_file: parsed_args.log_file,
    }
}
