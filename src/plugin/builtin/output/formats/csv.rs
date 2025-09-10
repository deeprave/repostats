//! CSV output formatter

use super::{FormatResult, OutputFormatter};
use crate::plugin::builtin::output::traits::ExportFormat;
use crate::plugin::data_export::{DataPayload, PluginDataExport, Value};
use crate::plugin::error::PluginResult;

/// CSV formatter implementation
pub struct CsvFormatter {
    delimiter: char,
}

impl CsvFormatter {
    /// Create a new CSV formatter
    pub fn new() -> Self {
        Self { delimiter: ',' }
    }

    /// Create a TSV formatter
    pub fn new_tsv() -> Self {
        Self { delimiter: '\t' }
    }

    /// Escape CSV value if needed
    fn escape_csv_value(&self, value: &str) -> String {
        if value.contains(self.delimiter) || value.contains('"') || value.contains('\n') {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_string()
        }
    }

    /// Format a value for CSV output
    fn format_value(&self, value: &Value) -> String {
        let str_value = match value {
            Value::String(s) => s.clone(),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Timestamp(ts) => format!("{:?}", ts), // TODO: Better timestamp formatting
            Value::Duration(d) => format!("{:?}", d),
            Value::Null => String::new(),
        };

        self.escape_csv_value(&str_value)
    }

    /// Format tabular data as CSV
    fn format_tabular(&self, rows: &[crate::plugin::data_export::Row]) -> PluginResult<String> {
        if rows.is_empty() {
            return Ok(String::new());
        }

        let mut result = String::new();

        // Determine max columns
        let max_cols = rows.iter().map(|r| r.values.len()).max().unwrap_or(0);

        // Add data rows
        for row in rows {
            let formatted_values: Vec<String> = (0..max_cols)
                .map(|i| {
                    if i < row.values.len() {
                        self.format_value(&row.values[i])
                    } else {
                        String::new()
                    }
                })
                .collect();

            result.push_str(&formatted_values.join(&self.delimiter.to_string()));
            result.push('\n');
        }

        Ok(result)
    }

    /// Convert hierarchical data to flat CSV format
    fn format_hierarchical(
        &self,
        tree: &crate::plugin::data_export::TreeNode,
    ) -> PluginResult<String> {
        let mut rows = Vec::new();
        self.flatten_tree(tree, String::new(), &mut rows);

        if rows.is_empty() {
            return Ok(String::new());
        }

        let mut result = String::new();

        // Add flattened data
        for (path, value) in rows {
            let escaped_path = self.escape_csv_value(&path);
            let escaped_value = self.escape_csv_value(&value);
            result.push_str(&format!(
                "{}{}{}\n",
                escaped_path, self.delimiter, escaped_value
            ));
        }

        Ok(result)
    }

    /// Recursively flatten tree structure
    fn flatten_tree(
        &self,
        node: &crate::plugin::data_export::TreeNode,
        path: String,
        rows: &mut Vec<(String, String)>,
    ) {
        let current_path = if path.is_empty() {
            node.key.clone()
        } else {
            format!("{}.{}", path, node.key)
        };

        rows.push((current_path.clone(), self.format_value(&node.value)));

        for child in &node.children {
            self.flatten_tree(child, current_path.clone(), rows);
        }
    }

    /// Format key-value data as CSV
    fn format_key_value(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> PluginResult<String> {
        let mut result = String::new();

        // Add data
        let mut keys: Vec<_> = data.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(value) = data.get(key) {
                let escaped_key = self.escape_csv_value(key);
                let escaped_value = self.escape_csv_value(&self.format_value(value));
                result.push_str(&format!(
                    "{}{}{}\n",
                    escaped_key, self.delimiter, escaped_value
                ));
            }
        }

        Ok(result)
    }
}

impl OutputFormatter for CsvFormatter {
    fn format(&self, data: &PluginDataExport, _use_colors: bool) -> FormatResult {
        // CSV doesn't use colors
        match &data.payload {
            DataPayload::Tabular { rows, .. } => self.format_tabular(rows),
            DataPayload::Hierarchical { roots } => {
                // Flatten all trees into a single hierarchy
                let mut combined_rows = Vec::new();
                for root in roots.iter() {
                    self.flatten_tree(root, String::new(), &mut combined_rows);
                }

                if combined_rows.is_empty() {
                    return Ok(String::new());
                }

                let mut result = String::new();

                for (path, value) in combined_rows {
                    let escaped_path = self.escape_csv_value(&path);
                    let escaped_value = self.escape_csv_value(&value);
                    result.push_str(&format!(
                        "{}{}{}\n",
                        escaped_path, self.delimiter, escaped_value
                    ));
                }

                Ok(result)
            }
            DataPayload::KeyValue { data } => self.format_key_value(data),
            DataPayload::Raw { data, .. } => {
                // For raw data, create a single-cell CSV
                Ok(self.escape_csv_value(data))
            }
        }
    }

    fn format_type(&self) -> ExportFormat {
        ExportFormat::Csv
    }
}
