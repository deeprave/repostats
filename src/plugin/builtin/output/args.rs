//! Argument parsing and format detection for OutputPlugin

use crate::plugin::args::PluginConfig;
use crate::plugin::data_export::ExportFormat;
use crate::plugin::error::{PluginError, PluginResult};
use std::path::Path;

/// Output plugin configuration derived from arguments
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// Output destination (file path or stdout)
    pub destination: OutputDestination,
    /// Output format
    pub format: ExportFormat,
    /// Whether to use colors (determined by PluginConfig, not set here)
    pub use_colors: bool,
    /// Whether to include headers (for formats that support them)
    pub include_headers: bool,
    /// Template path for template-based output
    pub template_path: Option<String>,
}

/// Output destination enum
#[derive(Debug, Clone, PartialEq)]
pub enum OutputDestination {
    /// Write to stdout
    Stdout,
    /// Write to a file
    File(String),
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            destination: OutputDestination::Stdout,
            format: ExportFormat::Console,
            use_colors: true,
            include_headers: true,
            template_path: None,
        }
    }
}

impl OutputConfig {
    /// Parse configuration from plugin arguments and CLI parsing
    /// This follows the same pattern as DumpPlugin - colors are handled by PluginConfig
    pub fn from_plugin_config_and_args(
        config: &PluginConfig,
        args: &std::collections::HashMap<String, String>,
        flags: &std::collections::HashSet<String>,
    ) -> PluginResult<Self> {
        let mut output_config = OutputConfig::default();

        // Parse output destination from args
        if let Some(output_arg) = args.get("output").or_else(|| args.get("o")) {
            output_config.destination = if output_arg == "-" || output_arg.is_empty() {
                OutputDestination::Stdout
            } else {
                OutputDestination::File(output_arg.clone())
            };
        }

        // Parse format - check format flags first, then explicit format
        if flags.contains("json") {
            output_config.format = ExportFormat::Json;
        } else if flags.contains("csv") {
            output_config.format = ExportFormat::Csv;
        } else if flags.contains("xml") {
            output_config.format = ExportFormat::Xml;
        } else if flags.contains("html") {
            output_config.format = ExportFormat::Html;
        } else if flags.contains("markdown") {
            output_config.format = ExportFormat::Markdown;
        } else if let Some(format_str) = args.get("format").or_else(|| args.get("f")) {
            output_config.format = parse_format_string(format_str)?;
        } else {
            // Auto-detect format from file extension if writing to file
            if let OutputDestination::File(ref path) = output_config.destination {
                output_config.format = detect_format_from_extension(path)?;
            }
        }

        // Template functionality not yet implemented - skip for now

        // Use colors from PluginConfig - startup code already handled TTY detection
        // Disable colors for file output regardless of config
        output_config.use_colors =
            if matches!(output_config.destination, OutputDestination::File(_)) {
                false
            } else {
                config.use_colors.unwrap_or(false)
            };

        // Parse header settings
        if flags.contains("no-headers") {
            output_config.include_headers = false;
        }

        Ok(output_config)
    }

    /// Check if this configuration suppresses progress output
    /// Progress is suppressed when writing to stdout (including '-')
    /// because progress would interfere with the data output
    pub fn suppresses_progress(&self) -> bool {
        matches!(self.destination, OutputDestination::Stdout)
    }

    /// Check if output destination is a terminal (for progress display logic)
    /// Only relevant when destination is stdout/'-'
    pub fn is_terminal_output(&self) -> bool {
        matches!(self.destination, OutputDestination::Stdout)
            && std::io::IsTerminal::is_terminal(&std::io::stdout())
    }

    /// Get the file extension for the configured format
    pub fn get_file_extension(&self) -> Option<&'static str> {
        self.format.file_extension()
    }
}

/// Parse format string into ExportFormat enum
fn parse_format_string(format_str: &str) -> PluginResult<ExportFormat> {
    match format_str.to_lowercase().as_str() {
        "json" => Ok(ExportFormat::Json),
        "csv" => Ok(ExportFormat::Csv),
        "tsv" => Ok(ExportFormat::Tsv),
        "xml" => Ok(ExportFormat::Xml),
        "html" => Ok(ExportFormat::Html),
        "markdown" | "md" => Ok(ExportFormat::Markdown),
        "console" | "text" => Ok(ExportFormat::Console),
        // "template" => Ok(ExportFormat::Template), // Not yet implemented
        _ => Err(PluginError::ConfigurationError {
            plugin_name: "OutputPlugin".to_string(),
            message: format!("Unsupported format: {}", format_str),
        }),
    }
}

/// Detect format from file extension
fn detect_format_from_extension(file_path: &str) -> PluginResult<ExportFormat> {
    let path = Path::new(file_path);
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    let format = match extension.as_str() {
        "json" => ExportFormat::Json,
        "csv" => ExportFormat::Csv,
        "tsv" => ExportFormat::Tsv,
        "xml" => ExportFormat::Xml,
        "html" | "htm" => ExportFormat::Html,
        "md" | "markdown" => ExportFormat::Markdown,
        "txt" | "log" => ExportFormat::Console,
        // "j2" | "tera" => ExportFormat::Template, // Not yet implemented
        "" => {
            return Err(PluginError::ConfigurationError {
                plugin_name: "OutputPlugin".to_string(),
                message: "Cannot determine format from file extension. Please specify --format"
                    .to_string(),
            });
        }
        _ => {
            return Err(PluginError::ConfigurationError {
                plugin_name: "OutputPlugin".to_string(),
                message: format!("Unsupported file extension: .{}", extension),
            });
        }
    };

    Ok(format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OutputConfig::default();
        assert_eq!(config.destination, OutputDestination::Stdout);
        assert_eq!(config.format, ExportFormat::Console);
        assert!(config.use_colors);
        assert!(config.include_headers);
        assert!(config.template_path.is_none());
    }

    #[test]
    fn test_parse_format_string() {
        assert_eq!(parse_format_string("json").unwrap(), ExportFormat::Json);
        assert_eq!(parse_format_string("CSV").unwrap(), ExportFormat::Csv);
        assert_eq!(parse_format_string("xml").unwrap(), ExportFormat::Xml);
        assert!(parse_format_string("invalid").is_err());
    }

    #[test]
    fn test_detect_format_from_extension() {
        assert_eq!(
            detect_format_from_extension("output.json").unwrap(),
            ExportFormat::Json
        );
        assert_eq!(
            detect_format_from_extension("data.csv").unwrap(),
            ExportFormat::Csv
        );
        assert_eq!(
            detect_format_from_extension("report.html").unwrap(),
            ExportFormat::Html
        );
        assert!(detect_format_from_extension("file").is_err());
        assert!(detect_format_from_extension("file.unknown").is_err());
    }

    #[test]
    fn test_suppresses_progress() {
        let stdout_config = OutputConfig {
            destination: OutputDestination::Stdout,
            ..Default::default()
        };
        assert!(stdout_config.suppresses_progress());

        let file_config = OutputConfig {
            destination: OutputDestination::File("output.json".to_string()),
            ..Default::default()
        };
        assert!(!file_config.suppresses_progress());
    }
}
