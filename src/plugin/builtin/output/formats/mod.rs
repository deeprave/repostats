//! Output formatting modules for the OutputPlugin
//!
//! This module provides format-specific implementations for different output types.
//! Each format is implemented in its own module to maintain separation of concerns.

pub mod csv;
pub mod html;
pub mod json;
pub mod markdown;
pub mod template;
pub mod text;
pub mod xml;

use crate::plugin::builtin::output::traits::{ExportFormat, FormatResult, OutputFormatter};
use crate::plugin::data_export::PluginDataExport;

/// Get formatter for the specified format
pub fn get_formatter(format: ExportFormat) -> Box<dyn OutputFormatter> {
    match format {
        ExportFormat::Json => Box::new(json::JsonFormatter::new()),
        ExportFormat::Console => Box::new(text::TextFormatter::new()),
        ExportFormat::Csv => Box::new(csv::CsvFormatter::new()),
        ExportFormat::Tsv => Box::new(csv::CsvFormatter::new_tsv()),
        ExportFormat::Xml => Box::new(xml::XmlFormatter::new()),
        ExportFormat::Html => Box::new(html::HtmlFormatter::new()),
        ExportFormat::Markdown => Box::new(markdown::MarkdownFormatter::new()),
        ExportFormat::Custom(ref format_name) if format_name == "template" => {
            Box::new(template::TemplateFormatter::new())
        }
        _ => Box::new(json::JsonFormatter::new()), // Default fallback
    }
}
