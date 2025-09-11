//! Output plugin traits and format definitions
//!
//! This module contains traits and types specific to the output plugin system.

use crate::plugin::data_export::PluginDataExport;
use crate::plugin::error::PluginResult;
use std::io::Write;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

/// Export format options for data output
#[derive(EnumIter, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// Text/console output
    Text,
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
}

impl ExportFormat {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Xml => "xml",
            Self::Html => "html",
            Self::Markdown => "md",
            Self::Yaml => "yaml",
            Self::Template => "j2",
        }
    }

    pub fn mimetype(&self) -> &'static str {
        match self {
            Self::Text => "text/plain",
            Self::Json => "application/json",
            Self::Csv => "text/csv",
            Self::Tsv => "text/tab-separated-values",
            Self::Xml => "application/xml",
            Self::Html => "text/html",
            Self::Markdown => "text/markdown",
            Self::Yaml => "application/x-yaml",
            Self::Template => "text/plain",
        }
    }

    pub fn aliases(&self) -> &'static [&'static str] {
        match self {
            Self::Text => &[
                "txt",
                "log",
                "out",
                "conf",
                "ini",
                "env",
                "properties",
                "rst",
            ],
            Self::Json => &["jsn", "geojson", "har", "map", "avsc", "json5", "jsonc"],
            Self::Csv => &[],
            Self::Tsv => &["tab"],
            Self::Xml => &[
                "xsd", "xlt", "dtd", "xsl", "rss", "atom", "svg", "gml", "project",
            ],
            Self::Html => &["htm", "xhtml", "xht", "xhtm", "shtml"],
            Self::Markdown => &["markdown", "mdown", "mkd", "mdx"],
            Self::Yaml => &["yml"],
            Self::Template => &["j2", "html", "tpl", "tmpl", "template", "jinja", "jinja2"],
        }
    }

    /// Public iterator over all ExportFormat variants (stable API surface)
    pub fn formats() -> impl Iterator<Item = ExportFormat> {
        ExportFormat::iter()
    }

    pub fn names() -> impl Iterator<Item = &'static str> {
        ExportFormat::iter().map(|fmt| fmt.name())
    }

    /// Get the default file extension for this format
    pub fn file_ext(&self) -> &'static str {
        self.name()
    }

    /// Get the common file extensions for this format
    pub fn file_exts(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.file_ext()).chain(self.aliases().iter().copied())
    }

    /// Parse format from string name (handles both CLI format strings and function names)
    pub fn from_str(s: &str) -> Self {
        let lowercase = s.to_lowercase();
        let ext = lowercase.as_str();
        for fmt in Self::iter() {
            // first match wins
            if fmt.name() == ext || fmt.aliases().contains(&ext) {
                return fmt.clone();
            }
        }
        log::warn!("Unknown format '{}', defaulting to text format", s);
        Self::Text
    }

    pub fn from_ext(ext: &str) -> Self {
        Self::from_str(ext)
    }

    /// Detect the format from the file path (using extension)
    pub fn from_file_path(path: &str) -> Self {
        let path = std::path::Path::new(path);
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(Self::from_str)
            .unwrap_or(Self::Text)
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
    /// Flush and finalise the output
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
    use std::collections::HashMap;

    #[test]
    fn test_export_format_variants_exist() {
        // Test all expected variants exist
        let text = ExportFormat::Text;
        let json = ExportFormat::Json;
        let csv = ExportFormat::Csv;
        let tsv = ExportFormat::Tsv;
        let xml = ExportFormat::Xml;
        let html = ExportFormat::Html;
        let markdown = ExportFormat::Markdown;
        let yaml = ExportFormat::Yaml;
        let template = ExportFormat::Template;

        assert_eq!(text, ExportFormat::Text);
        assert_eq!(json, ExportFormat::Json);
        assert_eq!(csv, ExportFormat::Csv);
        assert_eq!(tsv, ExportFormat::Tsv);
        assert_eq!(xml, ExportFormat::Xml);
        assert_eq!(html, ExportFormat::Html);
        assert_eq!(markdown, ExportFormat::Markdown);
        assert_eq!(yaml, ExportFormat::Yaml);
        assert_eq!(template, ExportFormat::Template);
    }

    #[test]
    fn test_export_format_file_extension() {
        assert_eq!(ExportFormat::Text.file_ext(), "text");
        assert_eq!(ExportFormat::Json.file_ext(), "json");
        assert_eq!(ExportFormat::Csv.file_ext(), "csv");
        assert_eq!(ExportFormat::Tsv.file_ext(), "tsv");
        assert_eq!(ExportFormat::Xml.file_ext(), "xml");
        assert_eq!(ExportFormat::Html.file_ext(), "html");
        assert_eq!(ExportFormat::Markdown.file_ext(), "md");
        assert_eq!(ExportFormat::Yaml.file_ext(), "yaml");
        assert_eq!(ExportFormat::Template.file_ext(), "j2");
    }

    #[test]
    fn test_export_format_from_extension() {
        assert_eq!(ExportFormat::from_ext("json"), ExportFormat::Json);
        assert_eq!(ExportFormat::from_ext("JSON"), ExportFormat::Json);
        assert_eq!(ExportFormat::from_ext("csv"), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_ext("tsv"), ExportFormat::Tsv);
        assert_eq!(ExportFormat::from_ext("xml"), ExportFormat::Xml);
        assert_eq!(ExportFormat::from_ext("html"), ExportFormat::Html);
        assert_eq!(ExportFormat::from_ext("md"), ExportFormat::Markdown);
        assert_eq!(ExportFormat::from_ext("yaml"), ExportFormat::Yaml);
        assert_eq!(ExportFormat::from_ext("j2"), ExportFormat::Template);
        assert_eq!(ExportFormat::from_ext("txt"), ExportFormat::Text);
    }

    #[test]
    fn test_export_format_mime_type() {
        assert_eq!(ExportFormat::Text.mimetype(), "text/plain");
        assert_eq!(ExportFormat::Json.mimetype(), "application/json");
        assert_eq!(ExportFormat::Csv.mimetype(), "text/csv");
        assert_eq!(ExportFormat::Tsv.mimetype(), "text/tab-separated-values");
        assert_eq!(ExportFormat::Xml.mimetype(), "application/xml");
        assert_eq!(ExportFormat::Html.mimetype(), "text/html");
        assert_eq!(ExportFormat::Markdown.mimetype(), "text/markdown");
        assert_eq!(ExportFormat::Yaml.mimetype(), "application/x-yaml");
        assert_eq!(ExportFormat::Template.mimetype(), "text/plain");
    }

    #[test]
    fn test_export_format_equality() {
        assert_eq!(ExportFormat::Json, ExportFormat::Json);
        assert_ne!(ExportFormat::Json, ExportFormat::Csv);
        assert_ne!(ExportFormat::Text, ExportFormat::Html);
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
        let original = ExportFormat::Json;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_export_format_from_str() {
        assert_eq!(ExportFormat::from_str("json"), ExportFormat::Json);
        assert_eq!(ExportFormat::from_str("JSON"), ExportFormat::Json);
        assert_eq!(ExportFormat::from_str("csv"), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_str("xml"), ExportFormat::Xml);
        assert_eq!(ExportFormat::from_str("html"), ExportFormat::Html);
        assert_eq!(ExportFormat::from_str("md"), ExportFormat::Markdown);
        assert_eq!(ExportFormat::from_str("yaml"), ExportFormat::Yaml);
        assert_eq!(ExportFormat::from_str("j2"), ExportFormat::Template);
        assert_eq!(ExportFormat::from_str("txt"), ExportFormat::Text);
    }

    #[test]
    fn test_export_format_from_file_path() {
        assert_eq!(
            ExportFormat::from_file_path("output.json"),
            ExportFormat::Json
        );
        assert_eq!(
            ExportFormat::from_file_path("/path/to/file.csv"),
            ExportFormat::Csv
        );
        assert_eq!(
            ExportFormat::from_file_path("report.html"),
            ExportFormat::Html
        );
        assert_eq!(
            ExportFormat::from_file_path("no_extension"),
            ExportFormat::Text
        );
        assert_eq!(
            ExportFormat::from_file_path("file.unknown"),
            ExportFormat::Text
        );
    }
}
