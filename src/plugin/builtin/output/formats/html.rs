//! HTML output formatter

use super::{FormatResult, OutputFormatter};
use crate::plugin::data_export::{DataPayload, ExportFormat, PluginDataExport, Value};

/// HTML formatter implementation
pub struct HtmlFormatter {
    include_styles: bool,
}

impl HtmlFormatter {
    /// Create a new HTML formatter
    pub fn new() -> Self {
        Self {
            include_styles: true,
        }
    }

    /// Escape HTML special characters
    fn escape_html(text: &str) -> String {
        text.chars()
            .map(|c| match c {
                '<' => "&lt;".to_string(),
                '>' => "&gt;".to_string(),
                '&' => "&amp;".to_string(),
                '"' => "&quot;".to_string(),
                '\'' => "&#39;".to_string(),
                _ => c.to_string(),
            })
            .collect()
    }

    /// Format a value for HTML display
    fn format_value(&self, value: &Value) -> String {
        match value {
            Value::String(s) => Self::escape_html(s),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Timestamp(ts) => format!("{:?}", ts),
            Value::Duration(d) => format!("{:?}", d),
            Value::Null => "<em>null</em>".to_string(),
        }
    }

    /// Get CSS styles for the HTML document
    fn get_styles(&self) -> &str {
        r#"
        <style>
            body {
                font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
                line-height: 1.6;
                color: #333;
                max-width: 1200px;
                margin: 0 auto;
                padding: 20px;
                background: #f5f5f5;
            }
            .container {
                background: white;
                border-radius: 8px;
                padding: 20px;
                box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            }
            h1 {
                color: #2c3e50;
                border-bottom: 2px solid #3498db;
                padding-bottom: 10px;
            }
            table {
                width: 100%;
                border-collapse: collapse;
                margin: 20px 0;
            }
            th, td {
                padding: 12px;
                text-align: left;
                border-bottom: 1px solid #ddd;
            }
            th {
                background: #3498db;
                color: white;
                font-weight: 600;
            }
            tr:hover {
                background: #f8f9fa;
            }
            .tree {
                margin: 20px 0;
            }
            .tree-node {
                margin-left: 20px;
                padding: 5px 0;
            }
            .tree-key {
                font-weight: 600;
                color: #2c3e50;
            }
            .tree-value {
                color: #7f8c8d;
                margin-left: 10px;
            }
            .kv-container {
                margin: 20px 0;
            }
            .kv-item {
                display: flex;
                padding: 10px;
                border-bottom: 1px solid #ecf0f1;
            }
            .kv-key {
                font-weight: 600;
                color: #2c3e50;
                min-width: 200px;
            }
            .kv-value {
                color: #34495e;
            }
            .metadata {
                font-size: 0.9em;
                color: #7f8c8d;
                font-style: italic;
                margin-top: 5px;
            }
            .raw-content {
                background: #f8f9fa;
                border: 1px solid #dee2e6;
                border-radius: 4px;
                padding: 15px;
                font-family: 'Courier New', monospace;
                white-space: pre-wrap;
                word-wrap: break-word;
            }
        </style>
        "#
    }

    /// Format tabular data as HTML table
    fn format_tabular(&self, rows: &[crate::plugin::data_export::Row]) -> String {
        if rows.is_empty() {
            return "<p>No data available</p>".to_string();
        }

        let mut html = String::from("<table>\n");

        // Determine max columns
        let max_cols = rows.iter().map(|r| r.values.len()).max().unwrap_or(0);

        // Add headers
        html.push_str("  <thead>\n    <tr>\n");
        for i in 0..max_cols {
            html.push_str(&format!("      <th>Column {}</th>\n", i + 1));
        }
        html.push_str("    </tr>\n  </thead>\n");

        // Add data rows
        html.push_str("  <tbody>\n");
        for row in rows {
            html.push_str("    <tr>\n");
            for i in 0..max_cols {
                html.push_str("      <td>");
                if i < row.values.len() {
                    html.push_str(&self.format_value(&row.values[i]));
                }
                html.push_str("</td>\n");
            }
            html.push_str("    </tr>\n");

            // Add metadata if present
            if !row.metadata.is_empty() {
                html.push_str("    <tr>\n");
                html.push_str(&format!(
                    "      <td colspan=\"{}\" class=\"metadata\">",
                    max_cols
                ));
                html.push_str("Metadata: ");
                for (key, value) in &row.metadata {
                    html.push_str(&format!(
                        "{}: {}, ",
                        Self::escape_html(key),
                        Self::escape_html(value)
                    ));
                }
                html.push_str("</td>\n    </tr>\n");
            }
        }
        html.push_str("  </tbody>\n</table>");

        html
    }

    /// Format hierarchical data as nested HTML
    fn format_hierarchical(
        &self,
        tree: &crate::plugin::data_export::TreeNode,
        depth: usize,
    ) -> String {
        let mut html = String::new();
        let indent = "  ".repeat(depth);

        html.push_str(&format!("{}<div class=\"tree-node\">\n", indent));
        html.push_str(&format!(
            "{}  <span class=\"tree-key\">{}</span>\n",
            indent,
            Self::escape_html(&tree.key)
        ));
        html.push_str(&format!(
            "{}  <span class=\"tree-value\">{}</span>\n",
            indent,
            self.format_value(&tree.value)
        ));

        // Add metadata if present
        if !tree.metadata.is_empty() {
            html.push_str(&format!("{}  <div class=\"metadata\">", indent));
            for (key, value) in &tree.metadata {
                html.push_str(&format!(
                    "{}: {}, ",
                    Self::escape_html(key),
                    Self::escape_html(value)
                ));
            }
            html.push_str("</div>\n");
        }

        // Add children
        if !tree.children.is_empty() {
            html.push_str(&format!("{}  <div class=\"tree-children\">\n", indent));
            for child in &tree.children {
                html.push_str(&self.format_hierarchical(child, depth + 2));
            }
            html.push_str(&format!("{}  </div>\n", indent));
        }

        html.push_str(&format!("{}</div>\n", indent));
        html
    }

    /// Format key-value data as HTML
    fn format_key_value(&self, data: &std::collections::HashMap<String, Value>) -> String {
        let mut html = String::from("<div class=\"kv-container\">\n");

        let mut keys: Vec<_> = data.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(value) = data.get(key) {
                html.push_str("  <div class=\"kv-item\">\n");
                html.push_str(&format!(
                    "    <div class=\"kv-key\">{}</div>\n",
                    Self::escape_html(key)
                ));
                html.push_str(&format!(
                    "    <div class=\"kv-value\">{}</div>\n",
                    self.format_value(value)
                ));
                html.push_str("  </div>\n");
            }
        }

        html.push_str("</div>");
        html
    }

    /// Wrap content in HTML document structure
    fn wrap_document(&self, content: &str, title: &str) -> String {
        let mut html = String::from("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html.push_str("  <meta charset=\"UTF-8\">\n");
        html.push_str(
            "  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
        );
        html.push_str(&format!("  <title>{}</title>\n", Self::escape_html(title)));

        if self.include_styles {
            html.push_str(self.get_styles());
        }

        html.push_str("</head>\n<body>\n");
        html.push_str("  <div class=\"container\">\n");
        html.push_str(&format!("    <h1>{}</h1>\n", Self::escape_html(title)));
        html.push_str(content);
        html.push_str("\n  </div>\n");
        html.push_str("</body>\n</html>");

        html
    }
}

impl OutputFormatter for HtmlFormatter {
    fn format(&self, data: &PluginDataExport, _use_colors: bool) -> FormatResult {
        // HTML doesn't use terminal colors
        let content = match &data.payload {
            DataPayload::Tabular { rows, .. } => self.format_tabular(rows),
            DataPayload::Hierarchical { roots } => {
                let mut html = String::from("<div class=\"tree\">\n");
                for root in roots.iter() {
                    html.push_str(&self.format_hierarchical(root, 1));
                }
                html.push_str("</div>");
                html
            }
            DataPayload::KeyValue { data } => self.format_key_value(data),
            DataPayload::Raw { data, content_type } => {
                let mut html = String::from("<div class=\"raw-content\">\n");
                if let Some(ct) = content_type {
                    html.push_str(&format!(
                        "  <div class=\"metadata\">Content-Type: {}</div>\n",
                        Self::escape_html(ct)
                    ));
                }
                html.push_str(&format!("  <pre>{}</pre>\n", Self::escape_html(data)));
                html.push_str("</div>");
                html
            }
        };

        let title = &data.plugin_id;
        Ok(self.wrap_document(&content, title))
    }

    fn format_type(&self) -> ExportFormat {
        ExportFormat::Html
    }
}
