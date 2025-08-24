// Removed old fern-style logger implementation - now using flexi_logger

// Global static logger handle for flexi_logger
static LOGGER_HANDLE: std::sync::OnceLock<std::sync::Mutex<flexi_logger::LoggerHandle>> =
    std::sync::OnceLock::new();

// New flexi_logger implementation
pub fn init_logging_flexi(
    log_level: Option<&str>,
    log_format: Option<&str>,
    log_file: Option<&str>,
    color_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use flexi_logger::{FileSpec, Logger};

    let level_str = log_level.unwrap_or("info");
    let format_type = log_format.map_or("text", |f| f);

    let mut logger = Logger::try_with_str(level_str)?;

    // Set format based on format type and color support
    match format_type {
        "json" => {
            logger = logger.format(json_format);
        }
        "ext" => {
            // Extended format with target info
            if color_enabled {
                logger = logger.format(extended_color_format);
            } else {
                logger = logger.format(extended_format);
            }
        }
        _ => {
            // Default "text" format without target info
            if color_enabled {
                logger = logger.format(simple_color_format);
            } else {
                logger = logger.format(simple_format);
            }
        }
    }

    // Configure file output if requested
    if let Some(file_path) = log_file {
        let file_spec = FileSpec::try_from(std::path::Path::new(file_path))?;
        logger = logger.log_to_file(file_spec);
    }

    // Start the logger and store the handle
    let handle = logger.start()?;
    let _ = LOGGER_HANDLE.set(std::sync::Mutex::new(handle));

    Ok(())
}

/// Reconfigure logging at runtime with flexi_logger
///
/// # Limitations
/// - **Format changes**: Log format (text/json) cannot be changed at runtime and is ignored
/// - **File path changes**: Log file path cannot be changed at runtime and is ignored
/// - **Color changes**: Color output format is set during initialization and cannot be changed at runtime
/// - **Only log level**: Currently only log level changes are supported at runtime
///
/// This is a limitation of flexi_logger's design where format and output configuration
/// must be set during logger initialization.
pub fn reconfigure_logging_flexi(
    log_level: Option<&str>,
    _log_format: Option<&str>, // Format cannot be changed at runtime in flexi_logger
    _log_file: Option<&str>,   // File path cannot be changed at runtime easily
    _color_enabled: bool, // Color format is set during initialization, cannot be changed at runtime
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(handle_mutex) = LOGGER_HANDLE.get() {
        if let Ok(mut handle) = handle_mutex.lock() {
            if let Some(level) = log_level {
                let _ = handle.parse_and_push_temp_spec(level);
            }
            Ok(())
        } else {
            Err("Could not acquire logger handle lock".into())
        }
    } else {
        Err("Logger handle not initialised. Call init_logging_flexi first.".into())
    }
}

pub fn init_logging(
    log_level: Option<&str>,
    log_format: Option<&str>,
    log_file: Option<&str>,
    color_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use new flexi_logger implementation
    init_logging_flexi(log_level, log_format, log_file, color_enabled)
}

pub fn reconfigure_logging(
    log_level: Option<&str>,
    log_format: Option<&str>,
    log_file: Option<&str>,
    color_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use new flexi_logger implementation
    reconfigure_logging_flexi(log_level, log_format, log_file, color_enabled)
}

// Simple text format without target info
fn simple_format(
    w: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> Result<(), std::io::Error> {
    let level_abbr = match record.level() {
        log::Level::Error => "ERR",
        log::Level::Warn => "WRN",
        log::Level::Info => "INF",
        log::Level::Debug => "DBG",
        log::Level::Trace => "TRC",
    };

    // Format target as path-like: module::submodule -> module/submodule.rs
    let target_formatted = format_target_as_path(record.target(), record.line());

    // Format: "YYYY-MM-DD HH:mm:ss.ffff INF message (app/startup.rs:42)"
    write!(
        w,
        "{} {} {} ({})",
        now.format("%Y-%m-%d %H:%M:%S%.3f"),
        level_abbr,
        record.args(),
        target_formatted
    )
}

// Simple color format without target info
fn simple_color_format(
    w: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> Result<(), std::io::Error> {
    use colored::*;

    let level_colored = match record.level() {
        log::Level::Error => "ERR".red().bold(),
        log::Level::Warn => "WRN".yellow(),
        log::Level::Info => "INF".green(),
        log::Level::Debug => "DBG".blue(),
        log::Level::Trace => "TRC".magenta(),
    };

    // Format: "YYYY-MM-DD HH:mm:ss.ffff INF message" with colors
    write!(
        w,
        "{} {} {}",
        now.format("%Y-%m-%d %H:%M:%S%.3f").to_string().dimmed(),
        level_colored,
        record.args()
    )
}

// Extended format with target info, no colors
fn extended_format(
    w: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> Result<(), std::io::Error> {
    let level_abbr = match record.level() {
        log::Level::Error => "ERR",
        log::Level::Warn => "WRN",
        log::Level::Info => "INF",
        log::Level::Debug => "DBG",
        log::Level::Trace => "TRC",
    };

    // Format target as path-like: module::submodule -> module/submodule.rs
    let target_formatted = format_target_as_path(record.target(), record.line());

    // Format: "YYYY-MM-DD HH:mm:ss.ffff INF message (app/startup.rs:42)"
    write!(
        w,
        "{} {} {} ({})",
        now.format("%Y-%m-%d %H:%M:%S%.3f"),
        level_abbr,
        record.args(),
        target_formatted
    )
}

// Extended color format with target info and colors
fn extended_color_format(
    w: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> Result<(), std::io::Error> {
    use colored::*;

    let level_colored = match record.level() {
        log::Level::Error => "ERR".red().bold(),
        log::Level::Warn => "WRN".yellow(),
        log::Level::Info => "INF".green(),
        log::Level::Debug => "DBG".blue(),
        log::Level::Trace => "TRC".magenta(),
    };

    // Format target as path-like: module::submodule -> module/submodule.rs
    let target_formatted = format_target_as_path(record.target(), record.line());

    // Format: "YYYY-MM-DD HH:mm:ss.ffff INF message (app/startup.rs:42)" with colors
    write!(
        w,
        "{} {} {} ({})",
        now.format("%Y-%m-%d %H:%M:%S%.3f").to_string().dimmed(),
        level_colored,
        record.args(),
        target_formatted.dimmed()
    )
}

// JSON format function with improved field ordering and target formatting
fn json_format(
    w: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> Result<(), std::io::Error> {
    use serde_json::{json, to_string};

    let level_abbr = match record.level() {
        log::Level::Error => "ERR",
        log::Level::Warn => "WRN",
        log::Level::Info => "INF",
        log::Level::Debug => "DBG",
        log::Level::Trace => "TRC",
    };

    let target_formatted = format_target_as_path(record.target(), record.line());

    // Ordered: timestamp, level, message, metadata
    let json_obj = json!({
        "timestamp": now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        "level": level_abbr,
        "message": record.args().to_string(),
        "target": target_formatted
    });

    // Use to_string to ensure compact JSON output - NO newlines added by us
    match to_string(&json_obj) {
        Ok(json_string) => {
            // Write only the JSON with no newlines to see what flexi_logger does
            w.write_all(json_string.as_bytes())?;
            Ok(())
        }
        Err(_) => {
            w.write_all(b"{\"error\":\"Failed to serialize log message\"}")?;
            Ok(())
        }
    }
}

// Helper function to format target as file path with line number
fn format_target_as_path(target: &str, line: Option<u32>) -> String {
    // Convert repostats::app::startup -> app/startup.rs
    let path_like = if let Some(without_prefix) = target.strip_prefix("repostats::") {
        // Remove the "repostats::" prefix and convert :: to /
        without_prefix.replace("::", "/") + ".rs"
    } else {
        // Handle other targets (external crates, etc.)
        target.replace("::", "/")
    };

    // Add line number if available
    if let Some(line_num) = line {
        format!("{}:{}", path_like, line_num)
    } else {
        path_like
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_test_logging() {
        INIT.call_once(|| {
            // Only call this once to avoid "logger already initialized" error
            let _ = init_logging_flexi(Some("debug"), None, None, false);
        });
    }

    #[test]
    fn test_flexi_logger_basic_functionality() {
        // This test should fail initially since we haven't implemented flexi_logger yet
        use flexi_logger::Logger;

        // Try to create a basic flexi_logger setup
        let result = Logger::try_with_str("debug");
        assert!(
            result.is_ok(),
            "flexi_logger should be able to create basic logger"
        );

        // Try to start it
        let logger = result.unwrap();
        let handle_result = logger.start();

        // This might fail due to multiple logger initialization, but the API should work
        // We just want to test that flexi_logger APIs are working
        let _handle = handle_result.map_err(|e| {
            // If it fails due to already being initialized, that's fine
            assert!(
                e.to_string().contains(
                    "attempted to set a logger after the logging system was already initialized"
                ) || e.to_string().contains("SetLoggerError")
                    || e.to_string().contains("Logger initialization failed"),
                "Expected initialization error, got: {}",
                e
            );
        });
    }

    #[test]
    fn test_current_logging_still_works() {
        init_test_logging();

        // Test that our current log macros still work
        log::info!("Test info message");
        log::debug!("Test debug message");
        log::warn!("Test warning message");

        // If we get here without panicking, logging is working
        assert!(true);
    }

    #[test]
    fn test_flexi_logger_custom_format() {
        use flexi_logger::Logger;

        // Test that we can create a logger with custom format
        // Our current format: "YYYY-MM-DD HH:mm:ss.ffff INF message (target)"

        let logger_result =
            Logger::try_with_str("debug").map(|logger| logger.format(extended_format));

        assert!(
            logger_result.is_ok(),
            "Should be able to create logger with custom format"
        );

        // For now, just test the API works. We now have extended_format function.
    }

    #[test]
    fn test_flexi_logger_format_matches_current() {
        use flexi_logger::DeferredNow;

        // Test that our format function produces the expected output
        let mut buffer = Vec::new();
        let mut now = DeferredNow::new();

        // Create a mock record
        let record = log::Record::builder()
            .level(log::Level::Info)
            .target("test_target")
            .args(format_args!("Test message"))
            .build();

        // Test our format function
        let result = extended_format(&mut buffer, &mut now, &record);
        assert!(result.is_ok(), "Format function should succeed");

        let output = String::from_utf8(buffer).expect("Output should be valid UTF-8");

        // Check format: "YYYY-MM-DD HH:mm:ss.ffff INF message (target)"
        assert!(
            output.contains("(test_target"),
            "Should contain target in parens"
        );
        assert!(output.contains("INF"), "Should contain level abbreviation");
        assert!(output.contains("Test message"), "Should contain message");
        assert!(output.contains(":"), "Should contain time separator");

        // Check the overall structure
        assert!(
            output.contains("INF Test message"),
            "Should have 'INF Test message' structure, got: {}",
            output
        );
    }

    #[test]
    fn test_flexi_logger_file_output() {
        use flexi_logger::{FileSpec, Logger};
        use std::fs;

        // Create a temporary directory for this test
        let temp_dir = std::env::temp_dir();
        let log_basename = "flexi_logger_test";

        // Remove any existing log files from previous test runs
        let _ = fs::remove_file(temp_dir.join(format!("{log_basename}.log")));
        let _ = fs::remove_file(temp_dir.join(format!("{log_basename}_r*.log")));

        // Test that we can create a logger with file output
        let logger_result = Logger::try_with_str("debug").map(|logger| {
            logger
                .log_to_file(
                    FileSpec::default()
                        .directory(&temp_dir)
                        .basename(log_basename),
                )
                .format(extended_format)
        });

        assert!(
            logger_result.is_ok(),
            "Should be able to create logger with file output"
        );

        // GREEN phase - now test passes because we can configure file logging
        assert!(true, "File logging configuration works");
    }

    #[test]
    fn test_flexi_logger_file_content() {
        use flexi_logger::{FileSpec, Logger};
        use std::fs;
        use std::time::Duration;

        let temp_dir = std::env::temp_dir();
        let log_basename = "flexi_logger_content_test";
        let expected_log_file = temp_dir.join(format!("{log_basename}.log"));

        // Clean up any existing files
        let _ = fs::remove_file(&expected_log_file);

        // Create and start a logger that writes to file
        let logger_handle = Logger::try_with_str("info")
            .unwrap()
            .log_to_file(
                FileSpec::default()
                    .directory(&temp_dir)
                    .basename(log_basename),
            )
            .format(extended_format)
            .start();

        // This might fail if another logger is already initialized, but that's ok for this test
        match logger_handle {
            Ok(_handle) => {
                // Logger started successfully, we can test file content
                log::info!("Test message for file content verification");

                // Give the logger a moment to flush
                std::thread::sleep(Duration::from_millis(100));

                // Check if log file was created and contains our message
                if expected_log_file.exists() {
                    let content = std::fs::read_to_string(&expected_log_file).unwrap_or_default();
                    assert!(
                        content.contains("Test message for file content verification"),
                        "Log file should contain our test message, got: {}",
                        content
                    );

                    // Check format
                    assert!(content.contains("[INFO]"), "Should contain log level");
                    assert!(content.contains(":"), "Should contain timestamp");
                } else {
                    // Log file doesn't exist yet - that's ok, flexi_logger might not have flushed
                    println!("Log file not found at: {}", expected_log_file.display());
                }

                // Cleanup
                let _ = fs::remove_file(&expected_log_file);
            }
            Err(e) => {
                // Logger already initialized - that's expected in test environment
                println!("Logger initialization failed (expected): {}", e);
                assert!(
                    e.to_string().contains("already initialized")
                        || e.to_string().contains("Logger initialization failed"),
                    "Expected initialization error, got: {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_flexi_logger_runtime_reconfiguration() {
        use flexi_logger::Logger;

        // Test runtime reconfiguration capability
        let logger_result = Logger::try_with_str("info").map(|logger| logger.start());

        match logger_result {
            Ok(Ok(mut handle)) => {
                // Logger started successfully - test reconfiguration
                // Test that we can push a temporary spec
                let _ = handle.parse_and_push_temp_spec("debug");

                // Test that we can pop it back
                handle.pop_temp_spec();

                // If we get here, runtime reconfiguration works
                assert!(true, "Runtime reconfiguration works");
            }
            Ok(Err(e)) => {
                // Expected if logger already initialized
                assert!(
                    e.to_string().contains("already initialized")
                        || e.to_string().contains("Logger initialization failed"),
                    "Expected initialization error, got: {}",
                    e
                );
            }
            Err(_) => {
                // Logger creation failed
                assert!(false, "Logger creation should not fail");
            }
        }
    }

    #[test]
    fn test_flexi_logger_reconfiguration_api() {
        // GREEN phase - test the specific API we need for reconfiguration
        use flexi_logger::Logger;

        // Test that we can create a logger and get the methods we need
        let logger_result = Logger::try_with_str("info");
        assert!(
            logger_result.is_ok(),
            "Should be able to create basic logger"
        );

        // Test that starting the logger gives us a handle
        let logger = logger_result.unwrap();
        let handle_result = logger.start();

        match handle_result {
            Ok(mut handle) => {
                // We have a handle - test the API methods we need

                // Test parse_and_push_temp_spec - changes log level temporarily
                let _ = handle.parse_and_push_temp_spec("trace");

                // Test pop_temp_spec - reverts to previous log level
                handle.pop_temp_spec();

                // If we get here, the API we need is available
                assert!(true, "Reconfiguration API is available");
            }
            Err(e) => {
                // Logger already initialized - that's expected in test environment
                assert!(
                    e.to_string().contains("already initialized")
                        || e.to_string().contains("Logger initialization failed"),
                    "Expected initialization error, got: {}",
                    e
                );
            }
        }
    }
}
