use log::{Log, Metadata, Record, LevelFilter};
use std::sync::{Arc, Mutex};
use std::fs::{File, OpenOptions};
use std::io::Write;
use chrono;

#[derive(Clone)]
struct LogConfig {
    level: LevelFilter,
    format_json: bool,
    file_path: Option<String>,
    color_enabled: bool,
}

struct FernStyleLogger {
    config: Arc<Mutex<LogConfig>>,
    file_writer: Arc<Mutex<Option<File>>>,
}

impl FernStyleLogger {
    fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(LogConfig {
                level: LevelFilter::Info,
                format_json: false,
                file_path: None,
                color_enabled: true,
            })),
            file_writer: Arc::new(Mutex::new(None)),
        }
    }

    fn reconfigure(&self,
                   log_level: Option<&str>,
                   log_format: Option<&str>,
                   log_file: Option<&str>,
                   color_enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let level = match log_level {
            Some(level_str) => match level_str.to_lowercase().as_str() {
                "trace" => LevelFilter::Trace,
                "debug" => LevelFilter::Debug,
                "info" => LevelFilter::Info,
                "warn" => LevelFilter::Warn,
                "error" => LevelFilter::Error,
                "off" => LevelFilter::Off,
                _ => LevelFilter::Info,
            },
            None => LevelFilter::Info,
        };

        let format_json = log_format == Some("json");
        let file_path = log_file.map(|s| s.to_string());

        // Handle file writer changes
        match &file_path {
            Some(path) => {
                // Open/reopen file
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                *self.file_writer.lock().unwrap() = Some(file);
            },
            None => {
                // Close file if no path specified
                *self.file_writer.lock().unwrap() = None;
            }
        }

        // Update config with the actual file_path value
        *self.config.lock().unwrap() = LogConfig {
            level,
            format_json,
            file_path, // Now this correctly reflects the current file path
            color_enabled,
        };

        // Update global max level
        log::set_max_level(level);

        Ok(())
    }

    fn format_console_message(&self, record: &Record, config: &LogConfig) -> String {
        if config.format_json {
            format!(
                r#"{{"timestamp":"{}","level":"{}","target":"{}","message":"{}"}}"#,
                chrono::Local::now().to_rfc3339(),
                record.level(),
                record.target(),
                record.args()
            )
        } else if config.color_enabled {
            // Use fern-style colours
            let colors = fern::colors::ColoredLevelConfig::new()
                .info(fern::colors::Color::Green)
                .warn(fern::colors::Color::Yellow)
                .error(fern::colors::Color::Red)
                .debug(fern::colors::Color::Blue)
                .trace(fern::colors::Color::Magenta);

            format!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.target(),
                colors.color(record.level()),
                record.args()
            )
        } else {
            format!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.target(),
                record.level(),
                record.args()
            )
        }
    }

    fn format_file_message(&self, record: &Record, config: &LogConfig) -> String {
        if config.format_json {
            format!(
                r#"{{"timestamp":"{}","level":"{}","target":"{}","message":"{}"}}"#,
                chrono::Local::now().to_rfc3339(),
                record.level(),
                record.target(),
                record.args()
            )
        } else {
            format!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.target(),
                record.level(),
                record.args()
            )
        }
    }
}

impl Log for FernStyleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let config = self.config.lock().unwrap();
        metadata.level() <= config.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let config = self.config.lock().unwrap();

        // Console output
        let console_message = self.format_console_message(record, &config);
        println!("{}", console_message);

        // File output (only if the file_path is set, and file_writer exists)
        if config.file_path.is_some() {
            if let Ok(mut file_opt) = self.file_writer.lock() {
                if let Some(ref mut file) = file_opt.as_mut() {
                    let file_message = self.format_file_message(record, &config);
                    let _ = writeln!(file, "{}", file_message);
                    let _ = file.flush();
                }
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut file_opt) = self.file_writer.lock() {
            if let Some(ref mut file) = file_opt.as_mut() {
                let _ = file.flush();
            }
        }
    }
}


// Global static logger
static LOGGER: std::sync::OnceLock<FernStyleLogger> = std::sync::OnceLock::new();


pub fn init_logging(
    log_level: Option<&str>,
    log_format: Option<&str>,
    log_file: Option<&str>,
    color_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {

    let logger = LOGGER.get_or_init(|| FernStyleLogger::new());

    // Set as the global logger (only works once)
    log::set_logger(logger)?;

    // Configure it
    logger.reconfigure(log_level, log_format, log_file, color_enabled)?;

    Ok(())
}

pub fn reconfigure_logging(
    log_level: Option<&str>,
    log_format: Option<&str>,
    log_file: Option<&str>,
    color_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(logger) = LOGGER.get() {
        logger.reconfigure(log_level, log_format, log_file, color_enabled)?;

        Ok(())
    } else {
        Err("Logger is not initialised. Call init_logging first.".into())
    }
}
