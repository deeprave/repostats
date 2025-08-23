//! CLI module containing argument parsing and related functionality

use crate::app::cli::initial_args::InitialArgs;
use std::path::PathBuf;

pub mod api;
pub mod command_segmenter;
pub mod global_args;
pub mod initial_args;

pub struct InitialArgsBundle {
    pub command_name: String,
    pub config_file: Option<PathBuf>,
    pub plugin_dir: Option<String>,
    pub plugin_exclude: Option<String>,
    pub color: bool,
    pub no_color: bool,
    pub log_format: Option<String>,
    pub log_level: Option<String>,
    pub log_file: Option<PathBuf>,
}

pub fn initial_args() -> InitialArgsBundle {
    let args = InitialArgs::parse_from_env();
    InitialArgsBundle {
        command_name: InitialArgs::command_name(),
        config_file: args.config_file,
        plugin_dir: args.plugin_dir,
        plugin_exclude: args.plugin_exclude,
        color: args.color,
        no_color: args.no_color,
        log_format: args.log_format,
        log_level: args.log_level,
        log_file: args.log_file,
    }
}
