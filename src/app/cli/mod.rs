//! CLI module containing argument parsing and related functionality

use std::path::PathBuf;
use crate::app::cli::initial_args::InitialArgs;

pub mod api;
pub mod initial_args;
pub mod global_args;
pub mod command_segmenter;

pub fn initial_args() -> (
        String,
        Option<PathBuf>,
        Option<String>,
        Option<String>,
        bool,
        bool,
        Option<String>,
        Option<String>,
        Option<PathBuf>) {
    let args = InitialArgs::parse_from_env();
    (
        InitialArgs::command_name(),
        args.config_file,
        args.plugin_dir,
        args.plugin_exclude,
        args.color,
        args.no_color,
        args.log_format,
        args.log_level,
        args.log_file,
    )
}
