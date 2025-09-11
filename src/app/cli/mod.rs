//! CLI module containing argument parsing and related functionality

use std::path::PathBuf;

pub mod args;
pub mod config;
pub mod display;
pub mod parsing;
pub mod segmenter;
pub mod validation;

#[cfg(test)]
pub mod tests;

// Re-export commonly used types
pub use args::CheckoutSettings;

pub struct RequiredArgs {
    pub global_args: Vec<String>,
    pub config_file: Option<PathBuf>,
    pub plugin_dirs: Vec<String>,
    pub plugin_exclusions: Vec<String>,
    pub color: Option<bool>,
    pub log_format: Option<String>,
    pub log_level: Option<String>,
    pub log_file: Option<PathBuf>,
    pub plugins: bool,
}

pub fn initial_args(command_name: &str) -> Result<RequiredArgs, crate::app::startup::StartupError> {
    use args::Args;
    use std::env;

    // Get the command line arguments
    let cli_args: Vec<String> = env::args().collect();

    // Parse using Args in initial mode with proper error handling
    let (parsed_args, global_args) = Args::parse_initial(command_name, &cli_args)
        .map_err(|e| crate::app::startup::StartupError::ValidationFailed { error: e })?;

    Ok(RequiredArgs {
        global_args,
        config_file: parsed_args.config_file,
        plugin_dirs: parsed_args.plugin_dirs,
        plugin_exclusions: parsed_args.plugin_exclusions,
        color: parsed_args.color,
        log_format: parsed_args.log_format,
        log_level: parsed_args.log_level,
        log_file: parsed_args.log_file,
        plugins: parsed_args.plugins,
    })
}
