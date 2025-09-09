//! JSON output formatter

use super::{FormatResult, OutputFormatter};
use crate::plugin::data_export::{DataPayload, ExportFormat, PluginDataExport, Value};
use crate::plugin::error::PluginError;
use serde_json::{json, Map, Value as JsonValue};

/// JSON formatter implementation
pub struct JsonFormatter {
    pretty: bool,
}

impl JsonFormatter {
    /// Create a new JSON formatter
    pub fn new() -> Self {
        Self { pretty: true }
    }

    /// Create a compact JSON formatter
    pub fn new_compact() -> Self {
        Self { pretty: false }
    }

    /// Convert plugin Value to JSON Value
    fn convert_value(&self, value: &Value) -> JsonValue {
        match value {
            Value::String(s) => JsonValue::String(s.clone()),
            Value::Integer(i) => JsonValue::Number((*i).into()),
            Value::Float(f) => {
                if let Some(num) = serde_json::Number::from_f64(*f) {
                    JsonValue::Number(num)
                } else {
                    JsonValue::Null
                }
            }
            Value::Boolean(b) => JsonValue::Bool(*b),
            Value::Timestamp(ts) => {
                JsonValue::String(format!("{:?}", ts)) // TODO: Better timestamp formatting
            }
            Value::Duration(d) => JsonValue::String(format!("{:?}", d)),
            Value::Null => JsonValue::Null,
        }
    }

    /// Format tabular data as JSON
    fn format_tabular(&self, rows: &[crate::plugin::data_export::Row]) -> JsonValue {
        let json_rows: Vec<JsonValue> = rows
            .iter()
            .map(|row| {
                let mut obj = Map::new();
                for (i, value) in row.values.iter().enumerate() {
                    let key = format!("col_{}", i);
                    obj.insert(key, self.convert_value(value));
                }

                // Add metadata if present - metadata is not Optional
                for (key, value) in &row.metadata {
                    obj.insert(format!("meta_{}", key), JsonValue::String(value.clone()));
                }

                JsonValue::Object(obj)
            })
            .collect();

        JsonValue::Array(json_rows)
    }

    /// Format hierarchical data as JSON
    fn format_hierarchical(&self, tree: &crate::plugin::data_export::TreeNode) -> JsonValue {
        let mut obj = Map::new();
        obj.insert("key".to_string(), JsonValue::String(tree.key.clone()));
        obj.insert("value".to_string(), self.convert_value(&tree.value));

        if !tree.children.is_empty() {
            let children: Vec<JsonValue> = tree
                .children
                .iter()
                .map(|child| self.format_hierarchical(child))
                .collect();
            obj.insert("children".to_string(), JsonValue::Array(children));
        }

        // Add metadata if present - metadata is not Optional
        if !tree.metadata.is_empty() {
            let mut meta_obj = Map::new();
            for (key, value) in &tree.metadata {
                meta_obj.insert(key.clone(), JsonValue::String(value.clone()));
            }
            obj.insert("metadata".to_string(), JsonValue::Object(meta_obj));
        }

        JsonValue::Object(obj)
    }

    /// Format key-value data as JSON
    fn format_key_value(&self, data: &std::collections::HashMap<String, Value>) -> JsonValue {
        let mut obj = Map::new();
        for (key, value) in data {
            obj.insert(key.clone(), self.convert_value(value));
        }
        JsonValue::Object(obj)
    }
}

impl OutputFormatter for JsonFormatter {
    fn format(&self, data: &PluginDataExport, _use_colors: bool) -> FormatResult {
        // JSON doesn't use colors
        let json_value = match &data.payload {
            DataPayload::Tabular { rows, .. } => self.format_tabular(rows),
            DataPayload::Hierarchical { roots } => {
                if roots.len() == 1 {
                    self.format_hierarchical(&roots[0])
                } else {
                    let roots_json: Vec<JsonValue> = roots
                        .iter()
                        .map(|root| self.format_hierarchical(root))
                        .collect();
                    JsonValue::Array(roots_json)
                }
            }
            DataPayload::KeyValue { data } => self.format_key_value(data),
            DataPayload::Raw { data, content_type } => {
                json!({
                    "content": data.as_str(),
                    "content_type": content_type.as_deref().unwrap_or("text/plain")
                })
            }
        };

        let output = if self.pretty {
            serde_json::to_string_pretty(&json_value)
        } else {
            serde_json::to_string(&json_value)
        };

        output.map_err(|e| PluginError::LoadError {
            plugin_name: "OutputPlugin".to_string(),
            cause: format!("JSON formatting error: {}", e),
        })
    }

    fn format_type(&self) -> ExportFormat {
        ExportFormat::Json
    }
}
