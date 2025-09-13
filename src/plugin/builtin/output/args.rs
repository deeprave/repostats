//! Argument parsing and format detection for OutputPlugin

use super::OutputPlugin;
use crate::plugin::args::{PluginArgParser, PluginConfig};
use crate::plugin::builtin::output::traits::{ExportFormat, OutputDestination};
use crate::plugin::error::PluginResult;
use crate::plugin::traits::Plugin;
use clap::Arg;
use std::path::PathBuf;

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
impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            destination: OutputDestination::Stdout,
            format: ExportFormat::Text,
            use_colors: true,
            template_path: None,
        }
    }
}

impl OutputConfig {
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
        Some(self.format.file_ext())
    }
}

impl OutputPlugin {
    pub(super) async fn args_parse(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()> {
        // Detect format based on function name
        let function_name = args[0].clone();
        let detected_format = ExportFormat::from_str(&function_name);

        let info = self.plugin_info();
        let parser = PluginArgParser::new(
            &function_name,
            &info.description,
            &info.version,
            config.use_colors,
        )
        .arg(
            Arg::new("outfile")
                .short('o')
                .long("outfile")
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
        )
        .arg(
            Arg::new("tsv")
                .long("tsv")
                .action(clap::ArgAction::SetTrue)
                .help("Use TSV (Tab-Separated Values) output format"),
        );

        let matches = parser.parse(&args)?;

        // Parse output destination - check command line args first, then config
        if let Some(output_path) = matches.get_one::<String>("outfile") {
            self.output_destination = if output_path == "-" {
                Some("-".to_string())
            } else {
                Some(output_path.clone())
            };
        } else {
            // Check config for output setting
            let config_outfile = config.get_string("outfile", "");
            if !config_outfile.is_empty() {
                self.output_destination = if config_outfile == "-" {
                    Some("-".to_string())
                } else {
                    Some(config_outfile)
                };
            }
        }

        let format = if let Some(template_path) = matches.get_one::<PathBuf>("template-path") {
            // Template path provided - force template format and store path
            self.template_path = Some(template_path.to_string_lossy().to_string());
            ExportFormat::Template
        } else if matches.get_flag("json") {
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
            ExportFormat::Text
        } else if matches.get_flag("tsv") {
            ExportFormat::Tsv
        } else if let Some(format_str) = matches.get_one::<String>("format") {
            ExportFormat::from_str(format_str)
        } else if detected_format != ExportFormat::Text {
            // Format was detected from function invocation - use it
            detected_format
        } else {
            // Auto-detect format from file extension if writing to file
            if let Some(ref dest) = self.output_destination {
                if dest != "-" {
                    // Try to detect the format from file extension, fall back to JSON if unknown
                    let detected = ExportFormat::from_file_path(dest);
                    if matches!(detected, ExportFormat::Text) {
                        ExportFormat::Json
                    } else {
                        detected
                    }
                } else {
                    ExportFormat::Text
                }
            } else {
                ExportFormat::Text
            }
        };

        // Store the detected format in the plugin instance for future use
        self.detected_format = Some(format);

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
        assert_eq!(config.format, ExportFormat::Text);
        assert!(config.use_colors);
        assert!(config.template_path.is_none());
    }

    #[test]
    fn test_format_detection_methods() {
        // Test centralized format detection methods from ExportFormat
        assert_eq!(ExportFormat::from_str("json"), ExportFormat::Json);
        assert_eq!(ExportFormat::from_str("CSV"), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_str("xml"), ExportFormat::Xml);
        assert_eq!(ExportFormat::from_str("invalid"), ExportFormat::Text);

        // Test file path detection
        assert_eq!(
            ExportFormat::from_file_path("output.json"),
            ExportFormat::Json
        );
        assert_eq!(ExportFormat::from_file_path("data.csv"), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_file_path("data.tsv"), ExportFormat::Tsv);
        assert_eq!(
            ExportFormat::from_file_path("report.html"),
            ExportFormat::Html
        );
        assert_eq!(ExportFormat::from_file_path("file"), ExportFormat::Text);
        assert_eq!(
            ExportFormat::from_file_path("file.unknown"),
            ExportFormat::Text
        );
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
