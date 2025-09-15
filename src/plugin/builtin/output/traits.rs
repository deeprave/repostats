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
