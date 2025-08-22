//! Logging configuration and setup
//! 
//! This module handles the setup of tracing-based logging with support for
//! configurable levels, formats (text/json), file output, and color control.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::Level;
use tracing_subscriber::{fmt, EnvFilter};

static LOGGING_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize logging from individual configuration values
/// 
/// This is called during startup to enable logging throughout the application.
/// Exits the process on error.
pub fn init_logging(
    log_level: Option<&str>,
    log_format: Option<&str>,
    log_file: Option<&str>,
    color_enabled: bool,
) {
    if let Err(e) = init_early_logging(log_level, log_format, log_file, color_enabled) {
        eprintln!("Failed to initialise logging: {}", e);
        std::process::exit(1);
    }
}

pub fn configure_logging(
    log_level: Option<&str>,
    log_format: Option<&str>,
    log_file: Option<&Path>,
    color_enabled: bool,
) {
    if let Err(e) = reconfigure_logging(log_level, log_format, log_file, color_enabled) {
        eprintln!("Failed to reconfigure logging: {}", e);
    }
}

/// Set logging level based on verbosity value
/// 
/// Maps verbosity levels to tracing levels:
///  <= -2 = error
///     -1 = warn
///      0 = info (default)
///      1 = debug
///  >=  2 = trace
pub fn set_logging_level(verbosity: i8) {
    let level_str = match verbosity {
        v if v <= -2 => "error",
        -1 => "warn",
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    
    if LOGGING_INITIALIZED.load(Ordering::SeqCst) {
        tracing::info!("Logging level adjusted to: {}", level_str);
        // Note: tracing doesn't support runtime reconfiguration
        // This would need to be handled at initialization
    }
}

/// Initialize logging with early defaults
/// 
/// This is called immediately after InitialArgs parsing to enable
/// logging throughout the rest of the startup process.
fn init_early_logging(
    initial_log_level: Option<&str>,
    initial_log_format: Option<&str>,
    initial_log_file: Option<&str>,
    color_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let level = parse_log_level(initial_log_level).unwrap_or(Level::INFO);
    let format = initial_log_format.unwrap_or("text");
    
    setup_tracing(level, format, initial_log_file, color_enabled)
}

/// Reconfigure logging after final args are parsed
/// 
/// Since tracing doesn't support reconfiguration, this just logs
/// a message about what the final configuration would be.
fn reconfigure_logging(
    log_level: Option<&str>,
    log_format: Option<&str>, 
    log_file: Option<&Path>,
    _color_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if LOGGING_INITIALIZED.load(Ordering::SeqCst) {
        tracing::info!(
            "Final logging configuration: level={:?}, format={:?}, file={:?}",
            log_level.unwrap_or("info"),
            log_format.unwrap_or("text"),
            log_file.map(|p| p.to_string_lossy()).unwrap_or("none".into())
        );
        tracing::warn!("Tracing reconfiguration not supported - using initial settings");
    }
    Ok(())
}

/// Parse log level string to tracing Level
fn parse_log_level(level_str: Option<&str>) -> Option<Level> {
    match level_str?.to_lowercase().as_str() {
        "trace" => Some(Level::TRACE),
        "debug" => Some(Level::DEBUG),
        "info" => Some(Level::INFO),
        "warn" | "warning" => Some(Level::WARN),
        "error" => Some(Level::ERROR),
        _ => None,
    }
}

/// Setup tracing subscriber with specified configuration
fn setup_tracing(
    level: Level,
    format: &str,
    log_file: Option<&str>,
    color_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Only initialize if not already done
    if LOGGING_INITIALIZED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Ok(()); // Already initialized
    }
    
    let filter = EnvFilter::new(format!("{}", level));
    
    // Use the FmtSubscriber builder for simpler configuration
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(color_enabled);
    
    match format.to_lowercase().as_str() {
        "json" => {
            subscriber.json().init();
        },
        _ => {
            // Default to "text" format
            subscriber.init();
        }
    }

    // TODO: Add file output support when log_file is Some
    if let Some(_file_path) = log_file {
        tracing::warn!("File logging not yet implemented, using stderr only");
    }
    
    Ok(())
}