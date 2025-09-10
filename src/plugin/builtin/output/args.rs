//! Argument parsing and format detection for OutputPlugin

use super::OutputPlugin;
use crate::plugin::args::{PluginArgParser, PluginConfig};
use crate::plugin::data_export::ExportFormat;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::traits::Plugin; // for plugin_info()
use clap::Arg;
use std::path::{Path, PathBuf};

/// Output plugin configuration derived from arguments
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// Output destination (file path or stdout)
    pub destination: OutputDestination,
    /// Output format
    pub format: ExportFormat,
    /// Whether to use colors (determined by PluginConfig, not set here)
    pub use_colors: bool,
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

        // Use colors from PluginConfig - startup code already handled TTY detection
        // Disable colors for file output regardless of config
        output_config.use_colors =
            if matches!(output_config.destination, OutputDestination::File(_)) {
                false
            } else {
                config.use_colors.unwrap_or(false)
            };

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
        "yaml" => Ok(ExportFormat::Yaml),
        "text" => Ok(ExportFormat::Console),
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
        "j2" | "tera" => ExportFormat::Template,
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

impl OutputPlugin {
    pub(super) async fn args_parse(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()> {
        let info = self.plugin_info();
        let parser = PluginArgParser::new(
            &info.name,
            &info.description,
            &info.version,
            config.use_colors,
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .alias("outfile")
                .value_name("FILE")
                .help("Write output to FILE (use '-' for stdout)")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .help("Output format")
                .value_parser([
                    "json", "csv", "tsv", "xml", "html", "markdown", "yaml", "text",
                ]),
        )
        .arg(
            Arg::new("template-path")
                .short('t')
                .long("template-path")
                .value_name("TEMPLATE")
                .help("Template path for custom formatting")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .action(clap::ArgAction::SetTrue)
                .help("Use JSON output format"),
        )
        .arg(
            Arg::new("csv")
                .long("csv")
                .action(clap::ArgAction::SetTrue)
                .help("Use CSV output format"),
        )
        .arg(
            Arg::new("xml")
                .long("xml")
                .action(clap::ArgAction::SetTrue)
                .help("Use XML output format"),
        )
        .arg(
            Arg::new("html")
                .long("html")
                .action(clap::ArgAction::SetTrue)
                .help("Use HTML output format"),
        )
        .arg(
            Arg::new("markdown")
                .long("markdown")
                .action(clap::ArgAction::SetTrue)
                .help("Use Markdown output format"),
        )
        .arg(
            Arg::new("text")
                .long("text")
                .action(clap::ArgAction::SetTrue)
                .help("Use plain text output format"),
        );

        let matches = parser.parse(args)?;

        // Parse output destination - check command line args first, then config
        if let Some(output_path) = matches.get_one::<String>("output") {
            self.output_destination = if output_path == "-" {
                Some("-".to_string())
            } else {
                Some(output_path.clone())
            };
        } else {
            // Check config for output setting
            let config_output = config.get_string("output", "");
            if !config_output.is_empty() {
                self.output_destination = if config_output == "-" {
                    Some("-".to_string())
                } else {
                    Some(config_output)
                };
            }
        }

        // Parse template path first - this overrides all other format settings
        let _format = if let Some(template_path) = matches.get_one::<PathBuf>("template-path") {
            // Template path provided - force template format and store path
            self.template_path = Some(template_path.to_string_lossy().to_string());
            ExportFormat::Template
        } else {
            // No template path - check other format options
            if matches.get_flag("json") {
                ExportFormat::Json
            } else if matches.get_flag("csv") {
                ExportFormat::Csv
            } else if matches.get_flag("xml") {
                ExportFormat::Xml
            } else if matches.get_flag("html") {
                ExportFormat::Html
            } else if matches.get_flag("markdown") {
                ExportFormat::Markdown
            } else if matches.get_flag("text") {
                ExportFormat::Console
            } else if let Some(format_str) = matches.get_one::<String>("format") {
                parse_format_string(format_str)?
            } else {
                // Auto-detect format from file extension if writing to file
                if let Some(ref dest) = self.output_destination {
                    if dest != "-" {
                        detect_format_from_extension(dest)?
                    } else {
                        ExportFormat::Console
                    }
                } else {
                    ExportFormat::Console
                }
            }
        };

        Ok(())
    }
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
