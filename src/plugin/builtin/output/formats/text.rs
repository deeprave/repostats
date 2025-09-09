//! Text/Console output formatter

use super::{FormatResult, OutputFormatter};
use crate::core::styles::StyleRole;
use crate::plugin::data_export::{DataPayload, ExportFormat, PluginDataExport, Value};

/// Text formatter for console output
pub struct TextFormatter;

impl TextFormatter {
    /// Create a new text formatter
    pub fn new() -> Self {
        Self
    }

    /// Format a value as string
    fn format_value(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => format!("{:.2}", f),
            Value::Boolean(b) => b.to_string(),
            Value::Timestamp(ts) => format!("{:?}", ts), // TODO: Better timestamp formatting
            Value::Duration(d) => format!("{:?}", d),
            Value::Null => "null".to_string(),
        }
    }

    /// Apply styling if colors are enabled
    fn style_text(&self, text: &str, role: StyleRole, use_colors: bool) -> String {
        role.paint(text, use_colors)
    }

    /// Format tabular data as a table
    fn format_tabular(&self, rows: &[crate::plugin::data_export::Row], use_colors: bool) -> String {
        if rows.is_empty() {
            return "No data".to_string();
        }

        // Calculate column widths
        let max_cols = rows.iter().map(|r| r.values.len()).max().unwrap_or(0);
        let mut col_widths = vec![0; max_cols];

        for row in rows {
            for (i, value) in row.values.iter().enumerate() {
                let formatted = self.format_value(value);
                col_widths[i] = col_widths[i].max(formatted.len());
            }
        }

        let mut result = String::new();

        // Header separator
        let separator = col_widths
            .iter()
            .map(|&w| "-".repeat(w + 2))
            .collect::<Vec<_>>()
            .join("+");
        result.push_str(&format!("+{}+\n", separator));

        // Data rows
        for row in rows {
            let mut formatted_row = String::new();
            for (i, value) in row.values.iter().enumerate() {
                let formatted = self.format_value(value);
                let padded = format!(" {:<width$} ", formatted, width = col_widths[i]);
                formatted_row.push_str(&format!("|{}", padded));
            }
            formatted_row.push('|');
            result.push_str(&format!("{}\n", formatted_row));
        }

        // Footer separator
        result.push_str(&format!("+{}+\n", separator));

        self.style_text(&result, StyleRole::Valid, use_colors)
    }

    /// Format hierarchical data as indented text
    fn format_hierarchical(
        &self,
        tree: &crate::plugin::data_export::TreeNode,
        indent: usize,
        use_colors: bool,
    ) -> String {
        let mut result = String::new();
        let indent_str = "  ".repeat(indent);

        // Format node key and value
        let mut line = format!("{}{}", indent_str, tree.key);
        line.push_str(&format!(": {}", self.format_value(&tree.value)));

        line = self.style_text(&line, StyleRole::Header, use_colors);
        result.push_str(&format!("{}\n", line));

        // Add metadata if present
        for (key, value) in &tree.metadata {
            let meta_line = format!("{}  [{}]: {}", indent_str, key, value);
            let styled_meta = self.style_text(&meta_line, StyleRole::Dim, use_colors);
            result.push_str(&format!("{}\n", styled_meta));
        }

        // Format children
        for child in &tree.children {
            result.push_str(&self.format_hierarchical(child, indent + 1, use_colors));
        }

        result
    }

    /// Format key-value data
    fn format_key_value(
        &self,
        data: &std::collections::HashMap<String, Value>,
        use_colors: bool,
    ) -> String {
        let mut result = String::new();
        let mut keys: Vec<_> = data.keys().collect();
        keys.sort();

        let max_key_width = keys.iter().map(|k| k.len()).max().unwrap_or(0);

        for key in keys {
            if let Some(value) = data.get(key) {
                let formatted_value = self.format_value(value);
                let line = format!(
                    "{:<width$}: {}",
                    key,
                    formatted_value,
                    width = max_key_width
                );

                let styled_key = self.style_text(
                    &format!("{:<width$}", key, width = max_key_width),
                    StyleRole::Header,
                    use_colors,
                );
                let styled_value = self.style_text(&formatted_value, StyleRole::Value, use_colors);
                let styled_line = format!("{}: {}", styled_key, styled_value);

                result.push_str(&format!("{}\n", styled_line));
            }
        }

        result
    }
}

impl OutputFormatter for TextFormatter {
    fn format(&self, data: &PluginDataExport, use_colors: bool) -> FormatResult {
        let output = match &data.payload {
            DataPayload::Tabular { rows, .. } => self.format_tabular(rows, use_colors),
            DataPayload::Hierarchical { roots } => {
                if roots.len() == 1 {
                    self.format_hierarchical(&roots[0], 0, use_colors)
                } else {
                    let mut result = String::new();
                    for (i, root) in roots.iter().enumerate() {
                        if i > 0 {
                            result.push_str("\n---\n");
                        }
                        result.push_str(&self.format_hierarchical(root, 0, use_colors));
                    }
                    result
                }
            }
            DataPayload::KeyValue { data } => self.format_key_value(data, use_colors),
            DataPayload::Raw { data, .. } => data.as_str().to_string(),
        };

        Ok(output)
    }

    fn format_type(&self) -> ExportFormat {
        ExportFormat::Console
    }
}
