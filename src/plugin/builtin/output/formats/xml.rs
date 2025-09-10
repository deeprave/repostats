//! XML output formatter

use super::{FormatResult, OutputFormatter};
use crate::plugin::builtin::output::traits::ExportFormat;
use crate::plugin::data_export::{DataPayload, PluginDataExport, Value};

/// XML formatter implementation
pub struct XmlFormatter {
    indent_size: usize,
}

impl XmlFormatter {
    /// Create a new XML formatter
    pub fn new() -> Self {
        Self { indent_size: 2 }
    }

    /// Escape XML special characters
    fn escape_xml(text: &str) -> String {
        text.chars()
            .map(|c| match c {
                '<' => "&lt;".to_string(),
                '>' => "&gt;".to_string(),
                '&' => "&amp;".to_string(),
                '"' => "&quot;".to_string(),
                '\'' => "&apos;".to_string(),
                _ => c.to_string(),
            })
            .collect()
    }

    /// Convert a Value to XML string representation
    fn value_to_xml(&self, value: &Value, element_name: &str, indent_level: usize) -> String {
        let indent = " ".repeat(indent_level * self.indent_size);
        let content = match value {
            Value::String(s) => Self::escape_xml(s),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Timestamp(ts) => format!("{:?}", ts), // TODO: Better timestamp formatting
            Value::Duration(d) => format!("{:?}", d),
            Value::Null => String::new(),
        };

        if content.is_empty() {
            format!("{}<{} />", indent, element_name)
        } else {
            format!("{}<{}>{}</{}>", indent, element_name, content, element_name)
        }
    }

    /// Format tabular data as XML
    fn format_tabular(&self, rows: &[crate::plugin::data_export::Row]) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<data type=\"tabular\">\n");

        for (row_idx, row) in rows.iter().enumerate() {
            let indent = " ".repeat(self.indent_size);
            xml.push_str(&format!("{}<!-- Row {} -->\n", indent, row_idx + 1));
            xml.push_str(&format!("{}<row>\n", indent));

            // Add values
            for (col_idx, value) in row.values.iter().enumerate() {
                let element_name = format!("col_{}", col_idx);
                xml.push_str(&self.value_to_xml(value, &element_name, 2));
                xml.push('\n');
            }

            // Add metadata if present
            if !row.metadata.is_empty() {
                let meta_indent = " ".repeat(2 * self.indent_size);
                xml.push_str(&format!("{}<metadata>\n", meta_indent));
                for (key, value) in &row.metadata {
                    let safe_key = Self::escape_xml(key);
                    let safe_value = Self::escape_xml(value);
                    xml.push_str(&format!(
                        "{}<{}>{}<!/{}>>\n",
                        " ".repeat(3 * self.indent_size),
                        safe_key,
                        safe_value,
                        safe_key
                    ));
                }
                xml.push_str(&format!("{}</metadata>\n", meta_indent));
            }

            xml.push_str(&format!("{}</row>\n", indent));
        }

        xml.push_str("</data>");
        xml
    }

    /// Format hierarchical data as XML
    fn format_hierarchical(
        &self,
        tree: &crate::plugin::data_export::TreeNode,
        indent_level: usize,
    ) -> String {
        let indent = " ".repeat(indent_level * self.indent_size);
        let mut xml = String::new();

        // Use key as element name (sanitized)
        let element_name = tree
            .key
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();

        xml.push_str(&format!(
            "{}<node name=\"{}\">\n",
            indent,
            Self::escape_xml(&tree.key)
        ));

        // Add value
        xml.push_str(&self.value_to_xml(&tree.value, "value", indent_level + 1));
        xml.push('\n');

        // Add metadata if present
        if !tree.metadata.is_empty() {
            let meta_indent = " ".repeat((indent_level + 1) * self.indent_size);
            xml.push_str(&format!("{}<metadata>\n", meta_indent));
            for (key, value) in &tree.metadata {
                xml.push_str(&format!(
                    "{}<{}>{}<!/{}>>\n",
                    " ".repeat((indent_level + 2) * self.indent_size),
                    Self::escape_xml(key),
                    Self::escape_xml(value),
                    Self::escape_xml(key)
                ));
            }
            xml.push_str(&format!("{}</metadata>\n", meta_indent));
        }

        // Add children
        if !tree.children.is_empty() {
            let children_indent = " ".repeat((indent_level + 1) * self.indent_size);
            xml.push_str(&format!("{}<children>\n", children_indent));
            for child in &tree.children {
                xml.push_str(&self.format_hierarchical(child, indent_level + 2));
            }
            xml.push_str(&format!("{}</children>\n", children_indent));
        }

        xml.push_str(&format!("{}</node>\n", indent));
        xml
    }

    /// Format key-value data as XML
    fn format_key_value(&self, data: &std::collections::HashMap<String, Value>) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<data type=\"key-value\">\n");

        let mut keys: Vec<_> = data.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(value) = data.get(key) {
                // Sanitize key for XML element name
                let element_name = key
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect::<String>();

                xml.push_str(&format!("  <!-- {} -->\n", Self::escape_xml(key)));
                xml.push_str(&self.value_to_xml(value, &element_name, 1));
                xml.push('\n');
            }
        }

        xml.push_str("</data>");
        xml
    }
}

impl OutputFormatter for XmlFormatter {
    fn format(&self, data: &PluginDataExport, _use_colors: bool) -> FormatResult {
        // XML doesn't use colors
        let output = match &data.payload {
            DataPayload::Tabular { rows, .. } => self.format_tabular(rows),
            DataPayload::Hierarchical { roots } => {
                let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
                xml.push_str("<data type=\"hierarchical\">\n");

                for root in roots.iter() {
                    xml.push_str(&self.format_hierarchical(root, 1));
                }

                xml.push_str("</data>");
                xml
            }
            DataPayload::KeyValue { data } => self.format_key_value(data),
            DataPayload::Raw { data, content_type } => {
                let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
                xml.push_str("<data type=\"raw\"");
                if let Some(ct) = content_type {
                    xml.push_str(&format!(" content-type=\"{}\"", Self::escape_xml(ct)));
                }
                xml.push_str(">\n");
                xml.push_str(&format!(
                    "  <content>{}</content>\n",
                    Self::escape_xml(data)
                ));
                xml.push_str("</data>");
                xml
            }
        };

        Ok(output)
    }

    fn format_type(&self) -> ExportFormat {
        ExportFormat::Xml
    }
}
