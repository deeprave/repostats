//! Output plugin traits and format definitions
//!
//! This module contains traits and types specific to the output plugin system.

use crate::plugin::data_export::PluginDataExport;
use crate::plugin::error::PluginResult;
use std::collections::HashMap;
use std::io::Write;

/// Export format options for data output
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// Console/terminal output with color support
    Console,
    /// JSON format
    Json,
    /// CSV (Comma Separated Values) format
    Csv,
    /// TSV (Tab Separated Values) format
    Tsv,
    /// XML format
    Xml,
    /// HTML format
    Html,
    /// Markdown format
    Markdown,
    /// YAML format
    Yaml,
    /// Template format (using Tera templates)
    Template,
    /// Custom format with identifier
    Custom(String),
}

impl ExportFormat {
    /// Get file extension for this format
    pub fn file_extension(&self) -> Option<&'static str> {
        match self {
            Self::Console => None,
            Self::Json => Some("json"),
            Self::Csv => Some("csv"),
            Self::Tsv => Some("tsv"),
            Self::Xml => Some("xml"),
            Self::Html => Some("html"),
            Self::Markdown => Some("md"),
            Self::Yaml => Some("yaml"),
            Self::Template => Some("j2"),
            Self::Custom(_) => None,
        }
    }

    /// Detect format from file extension
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "csv" => Some(Self::Csv),
            "tsv" => Some(Self::Tsv),
            "xml" => Some(Self::Xml),
            "html" | "htm" => Some(Self::Html),
            "md" | "markdown" => Some(Self::Markdown),
            "yaml" | "yml" => Some(Self::Yaml),
            "txt" | "text" | "log" => Some(Self::Console),
            _ => None,
        }
    }

    /// Parse format from string name (handles both CLI format strings and function names)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "csv" => Some(Self::Csv),
            "tsv" => Some(Self::Tsv),
            "xml" => Some(Self::Xml),
            "html" => Some(Self::Html),
            "markdown" | "md" => Some(Self::Markdown),
            "yaml" | "yml" => Some(Self::Yaml),
            "text" | "console" => Some(Self::Console),
            "template" => Some(Self::Template),
            _ => None,
        }
    }

    /// Detect format from file path (using extension)
    pub fn from_file_path(path: &str) -> Option<Self> {
        let path = std::path::Path::new(path);
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// Get MIME type for this format
    pub fn mime_type(&self) -> Option<&'static str> {
        match self {
            Self::Console => None,
            Self::Json => Some("application/json"),
            Self::Csv => Some("text/csv"),
            Self::Tsv => Some("text/tab-separated-values"),
            Self::Xml => Some("application/xml"),
            Self::Html => Some("text/html"),
            Self::Markdown => Some("text/markdown"),
            Self::Yaml => Some("application/x-yaml"),
            Self::Template => Some("text/plain"),
            Self::Custom(_) => None,
        }
    }
}

/// Output destination specification
#[derive(Debug, Clone, PartialEq)]
pub enum OutputDestination {
    /// Write to stdout
    Stdout,
    /// Write to a file
    File(String),
}

/// Format-specific output result type alias
pub type FormatResult = PluginResult<String>;

/// Output writer trait for abstraction over different output destinations
pub trait OutputWriter: Write {
    /// Flush and finalize the output
    fn finalize(&mut self) -> std::io::Result<()>;
}

/// Trait for output formatters
pub trait OutputFormatter {
    /// Format the provided data export into the target format
    /// use_colors parameter comes from PluginConfig processing
    fn format(&self, data: &PluginDataExport, use_colors: bool) -> FormatResult;

    /// Get the format type this formatter handles
    fn format_type(&self) -> ExportFormat;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_format_variants_exist() {
        // Test all expected variants exist
        let console = ExportFormat::Console;
        let json = ExportFormat::Json;
        let csv = ExportFormat::Csv;
        let tsv = ExportFormat::Tsv;
        let xml = ExportFormat::Xml;
        let html = ExportFormat::Html;
        let markdown = ExportFormat::Markdown;
        let yaml = ExportFormat::Yaml;
        let custom = ExportFormat::Custom("custom".to_string());

        assert_eq!(console, ExportFormat::Console);
        assert_eq!(json, ExportFormat::Json);
        assert_eq!(csv, ExportFormat::Csv);
        assert_eq!(tsv, ExportFormat::Tsv);
        assert_eq!(xml, ExportFormat::Xml);
        assert_eq!(html, ExportFormat::Html);
        assert_eq!(markdown, ExportFormat::Markdown);
        assert_eq!(yaml, ExportFormat::Yaml);
        assert_eq!(custom, ExportFormat::Custom("custom".to_string()));
    }

    #[test]
    fn test_export_format_file_extension() {
        assert_eq!(ExportFormat::Console.file_extension(), None);
        assert_eq!(ExportFormat::Json.file_extension(), Some("json"));
        assert_eq!(ExportFormat::Csv.file_extension(), Some("csv"));
        assert_eq!(ExportFormat::Tsv.file_extension(), Some("tsv"));
        assert_eq!(ExportFormat::Xml.file_extension(), Some("xml"));
        assert_eq!(ExportFormat::Html.file_extension(), Some("html"));
        assert_eq!(ExportFormat::Markdown.file_extension(), Some("md"));
        assert_eq!(ExportFormat::Yaml.file_extension(), Some("yaml"));
        assert_eq!(
            ExportFormat::Custom("test".to_string()).file_extension(),
            None
        );
    }

    #[test]
    fn test_export_format_from_extension() {
        assert_eq!(
            ExportFormat::from_extension("json"),
            Some(ExportFormat::Json)
        );
        assert_eq!(
            ExportFormat::from_extension("JSON"),
            Some(ExportFormat::Json)
        );
        assert_eq!(ExportFormat::from_extension("csv"), Some(ExportFormat::Csv));
        assert_eq!(ExportFormat::from_extension("tsv"), Some(ExportFormat::Tsv));
        assert_eq!(ExportFormat::from_extension("xml"), Some(ExportFormat::Xml));
        assert_eq!(
            ExportFormat::from_extension("html"),
            Some(ExportFormat::Html)
        );
        assert_eq!(
            ExportFormat::from_extension("htm"),
            Some(ExportFormat::Html)
        );
        assert_eq!(
            ExportFormat::from_extension("md"),
            Some(ExportFormat::Markdown)
        );
        assert_eq!(
            ExportFormat::from_extension("markdown"),
            Some(ExportFormat::Markdown)
        );
        assert_eq!(
            ExportFormat::from_extension("yaml"),
            Some(ExportFormat::Yaml)
        );
        assert_eq!(
            ExportFormat::from_extension("yml"),
            Some(ExportFormat::Yaml)
        );
        assert_eq!(ExportFormat::from_extension("unknown"), None);
    }

    #[test]
    fn test_export_format_mime_type() {
        assert_eq!(ExportFormat::Console.mime_type(), None);
        assert_eq!(ExportFormat::Json.mime_type(), Some("application/json"));
        assert_eq!(ExportFormat::Csv.mime_type(), Some("text/csv"));
        assert_eq!(
            ExportFormat::Tsv.mime_type(),
            Some("text/tab-separated-values")
        );
        assert_eq!(ExportFormat::Xml.mime_type(), Some("application/xml"));
        assert_eq!(ExportFormat::Html.mime_type(), Some("text/html"));
        assert_eq!(ExportFormat::Markdown.mime_type(), Some("text/markdown"));
        assert_eq!(ExportFormat::Yaml.mime_type(), Some("application/x-yaml"));
        assert_eq!(ExportFormat::Custom("test".to_string()).mime_type(), None);
    }

    #[test]
    fn test_export_format_equality() {
        assert_eq!(ExportFormat::Json, ExportFormat::Json);
        assert_ne!(ExportFormat::Json, ExportFormat::Csv);
        assert_eq!(
            ExportFormat::Custom("test".to_string()),
            ExportFormat::Custom("test".to_string())
        );
        assert_ne!(
            ExportFormat::Custom("test".to_string()),
            ExportFormat::Custom("other".to_string())
        );
    }

    #[test]
    fn test_export_format_hash() {
        let mut map = HashMap::new();
        map.insert(ExportFormat::Json, "json data");
        map.insert(ExportFormat::Csv, "csv data");

        assert_eq!(map.get(&ExportFormat::Json), Some(&"json data"));
        assert_eq!(map.get(&ExportFormat::Csv), Some(&"csv data"));
    }

    #[test]
    fn test_export_format_clone() {
        let original = ExportFormat::Custom("test".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_export_format_from_str() {
        assert_eq!(ExportFormat::from_str("json"), Some(ExportFormat::Json));
        assert_eq!(ExportFormat::from_str("JSON"), Some(ExportFormat::Json));
        assert_eq!(ExportFormat::from_str("csv"), Some(ExportFormat::Csv));
        assert_eq!(ExportFormat::from_str("xml"), Some(ExportFormat::Xml));
        assert_eq!(ExportFormat::from_str("html"), Some(ExportFormat::Html));
        assert_eq!(
            ExportFormat::from_str("markdown"),
            Some(ExportFormat::Markdown)
        );
        assert_eq!(ExportFormat::from_str("md"), Some(ExportFormat::Markdown));
        assert_eq!(ExportFormat::from_str("yaml"), Some(ExportFormat::Yaml));
        assert_eq!(
            ExportFormat::from_str("console"),
            Some(ExportFormat::Console)
        );
        assert_eq!(ExportFormat::from_str("text"), Some(ExportFormat::Console));
        assert_eq!(ExportFormat::from_str("invalid"), None);
    }

    #[test]
    fn test_export_format_from_file_path() {
        assert_eq!(
            ExportFormat::from_file_path("output.json"),
            Some(ExportFormat::Json)
        );
        assert_eq!(
            ExportFormat::from_file_path("/path/to/file.csv"),
            Some(ExportFormat::Csv)
        );
        assert_eq!(
            ExportFormat::from_file_path("report.html"),
            Some(ExportFormat::Html)
        );
        assert_eq!(ExportFormat::from_file_path("no_extension"), None);
        assert_eq!(ExportFormat::from_file_path("file.unknown"), None);
    }
}
