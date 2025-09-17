//! Template output formatter using Tera
//!
//! This provides a full-featured template system based on Tera (Jinja2-like syntax).
//! Supports all standard Tera features: filters, loops, conditionals, inheritance, macros, etc.

use super::{FormatResult, OutputFormatter};
use crate::plugin::builtin::output::traits::ExportFormat;
use crate::plugin::data_export::{DataPayload, PluginDataExport, Value};
use crate::plugin::error::{PluginError, PluginResult};
use std::collections::HashMap;
use tera::{Context, Tera};

/// Default timeout for HTTP requests when loading templates from URLs
pub const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 60;

/// Template formatter implementation using Tera template engine
pub struct TemplateFormatter {
    tera: Tera,
    template_name: String,
}

impl TemplateFormatter {
    /// Reserved context variable names that should not be overwritten by user data
    const RESERVED_CONTEXT_KEYS: &'static [&'static str] = &[
        "rows",
        "trees",
        "pairs",
        "content",
        "content_type",
        "timestamp",
        "repository",
        "row_count",
        "tree_count",
        "pair_count",
        "column_count",
        // Tera built-ins
        "loop",
        "self",
        "super",
        "block",
        "macro",
        "set",
        "if",
        "for",
        "with",
        // Common template variables
        "data",
        "metadata",
        "schema",
        "hints",
        "plugin_id",
        "scan_id",
    ];

    /// Create a new Template formatter with default templates
    pub fn new() -> Self {
        let mut tera = Tera::default(); // Use default instead of new with empty glob

        // Add default templates
        let _ = tera.add_raw_templates(vec![
            ("tabular_default", Self::get_default_tabular_template()),
            (
                "hierarchical_default",
                Self::get_default_hierarchical_template(),
            ),
            ("keyvalue_default", Self::get_default_keyvalue_template()),
            ("raw_default", Self::get_default_raw_template()),
        ]);

        Self {
            tera,
            template_name: "default".to_string(),
        }
    }

    /// Create a Template formatter with custom template content
    pub fn with_template(template: String) -> PluginResult<Self> {
        let mut tera = Tera::default();

        tera.add_raw_template("custom", &template).map_err(|e| {
            PluginError::ConfigurationError {
                plugin_name: "TemplateFormatter".to_string(),
                message: format!("Invalid template syntax: {}", e),
            }
        })?;

        Ok(Self {
            tera,
            template_name: "custom".to_string(),
        })
    }

    /// Load template from file path
    pub fn from_file(path: &str) -> PluginResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| PluginError::IoError {
            operation: "read template".to_string(),
            path: path.to_string(),
            source: Some(Box::new(e)),
        })?;

        Self::with_template(content)
    }

    /// Load template from URL with default timeout
    pub async fn from_url(url: &str) -> PluginResult<Self> {
        Self::from_url_with_timeout(url, DEFAULT_HTTP_TIMEOUT_SECS).await
    }

    /// Load template from URL with custom timeout
    pub async fn from_url_with_timeout(url: &str, timeout_secs: u64) -> PluginResult<Self> {
        // Simple URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(PluginError::ConfigurationError {
                plugin_name: "TemplateFormatter".to_string(),
                message: format!(
                    "Invalid URL scheme. Only http:// and https:// are supported: {}",
                    url
                ),
            });
        }

        // Create HTTP client with configurable timeout
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| PluginError::ConfigurationError {
                plugin_name: "TemplateFormatter".to_string(),
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        // Fetch template content from URL
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| PluginError::IoError {
                operation: "fetch template".to_string(),
                path: url.to_string(),
                source: Some(Box::new(e)),
            })?;

        // Check if response is successful
        if !response.status().is_success() {
            return Err(PluginError::IoError {
                operation: format!("fetch template (HTTP {})", response.status().as_u16())
                    .to_string(),
                path: url.to_string(),
                source: None,
            });
        }

        // Get response text
        let content = response.text().await.map_err(|e| PluginError::IoError {
            operation: "read template response".to_string(),
            path: url.to_string(),
            source: Some(Box::new(e)),
        })?;

        Self::with_template(content)
    }

    /// Load template from path or URL (auto-detect) with default timeout
    pub async fn from_source(source: &str) -> PluginResult<Self> {
        Self::from_source_with_timeout(source, DEFAULT_HTTP_TIMEOUT_SECS).await
    }

    /// Load template from path or URL (auto-detect) with custom timeout
    pub async fn from_source_with_timeout(source: &str, timeout_secs: u64) -> PluginResult<Self> {
        if source.starts_with("http://") || source.starts_with("https://") {
            Self::from_url_with_timeout(source, timeout_secs).await
        } else {
            Ok(Self::from_file(source)?)
        }
    }

    /// Safely insert user-provided key-value pair into context with collision prevention
    fn safe_insert_user_data(&self, context: &mut Context, key: &str, value: &serde_json::Value) {
        // Sanitise key for template variable name
        let safe_key = key
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();

        // Check for reserved keywords and namespace user data to prevent collisions
        let final_key = if Self::RESERVED_CONTEXT_KEYS.contains(&safe_key.as_str()) {
            format!("user_{}", safe_key) // Namespace reserved keywords
        } else if safe_key.is_empty() || safe_key.starts_with('_') {
            // Use original key hash to ensure uniqueness for problematic keys
            let key_hash = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                key.hash(&mut hasher);
                hasher.finish()
            };
            format!("user_data_{:x}", key_hash) // Use hex hash for unique identification
        } else {
            safe_key
        };

        context.insert(&final_key, value);
    }

    /// Convert plugin Value to Tera-compatible serde_json::Value
    fn value_to_tera_value(&self, value: &Value) -> serde_json::Value {
        match value {
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            Value::Float(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Value::Boolean(b) => serde_json::Value::Bool(*b),
            Value::Timestamp(ts) => {
                use std::time::UNIX_EPOCH;
                match ts.duration_since(UNIX_EPOCH) {
                    Ok(duration) => {
                        let secs = duration.as_secs();
                        let nanos = duration.subsec_nanos();

                        // Handle potential overflow/underflow gracefully
                        match chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, nanos) {
                            Some(dt) => {
                                // Include subsecond precision for full accuracy
                                serde_json::Value::String(
                                    dt.format("%Y-%m-%d %H:%M:%S%.6f UTC").to_string(),
                                )
                            }
                            None => {
                                // Timestamp out of range - fall back to debug representation
                                serde_json::Value::String(format!("Invalid timestamp: {:?}", ts))
                            }
                        }
                    }
                    Err(_) => {
                        // Timestamp before UNIX epoch - handle gracefully
                        serde_json::Value::String(format!("Pre-epoch timestamp: {:?}", ts))
                    }
                }
            }
            Value::Duration(d) => serde_json::Value::String(format!("{:?}", d)),
            Value::Null => serde_json::Value::Null,
        }
    }

    /// Add tabular data to Tera context
    fn add_tabular_context(&self, context: &mut Context, rows: &[crate::plugin::data_export::Row]) {
        context.insert("row_count", &rows.len());

        if !rows.is_empty() {
            let max_cols = rows.iter().map(|r| r.values.len()).max().unwrap_or(0);
            context.insert("column_count", &max_cols);

            // Convert rows to Tera-compatible format
            let tera_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let row_values: Vec<serde_json::Value> = row
                        .values
                        .iter()
                        .map(|v| self.value_to_tera_value(v))
                        .collect();

                    let mut row_obj = serde_json::Map::new();
                    row_obj.insert("values".to_string(), serde_json::Value::Array(row_values));

                    // Add row metadata
                    if !row.metadata.is_empty() {
                        let metadata: serde_json::Map<String, serde_json::Value> = row
                            .metadata
                            .iter()
                            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                            .collect();
                        row_obj.insert("metadata".to_string(), serde_json::Value::Object(metadata));
                    }

                    serde_json::Value::Object(row_obj)
                })
                .collect();

            context.insert("rows", &tera_rows);
        }
    }

    /// Add hierarchical data to Tera context
    fn add_hierarchical_context(
        &self,
        context: &mut Context,
        roots: &[crate::plugin::data_export::TreeNode],
    ) {
        context.insert("tree_count", &roots.len());

        // Convert tree nodes to Tera-compatible format
        let tera_trees: Vec<serde_json::Value> = roots
            .iter()
            .map(|root| self.tree_node_to_tera_value(root))
            .collect();

        context.insert("trees", &tera_trees);
    }

    /// Convert tree node to Tera-compatible serde_json::Value
    fn tree_node_to_tera_value(
        &self,
        node: &crate::plugin::data_export::TreeNode,
    ) -> serde_json::Value {
        let mut node_obj = serde_json::Map::new();

        node_obj.insert(
            "key".to_string(),
            serde_json::Value::String(node.key.clone()),
        );
        node_obj.insert("value".to_string(), self.value_to_tera_value(&node.value));

        // Add metadata if present
        if !node.metadata.is_empty() {
            let metadata: serde_json::Map<String, serde_json::Value> = node
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            node_obj.insert("metadata".to_string(), serde_json::Value::Object(metadata));
        }

        // Recursively convert children
        let children: Vec<serde_json::Value> = node
            .children
            .iter()
            .map(|child| self.tree_node_to_tera_value(child))
            .collect();
        node_obj.insert("children".to_string(), serde_json::Value::Array(children));

        serde_json::Value::Object(node_obj)
    }

    /// Add key-value data to Tera context
    fn add_keyvalue_context(&self, context: &mut Context, data: &HashMap<String, Value>) {
        context.insert("pair_count", &data.len());

        // Convert key-value pairs to Tera-compatible format
        let tera_pairs: Vec<serde_json::Value> = data
            .iter()
            .map(|(key, value)| {
                let mut pair_obj = serde_json::Map::new();
                pair_obj.insert("key".to_string(), serde_json::Value::String(key.clone()));
                pair_obj.insert("value".to_string(), self.value_to_tera_value(value));
                serde_json::Value::Object(pair_obj)
            })
            .collect();

        context.insert("pairs", &tera_pairs);

        // Also add each key-value pair as individual context variables for easy access
        for (key, value) in data {
            let tera_value = self.value_to_tera_value(value);
            self.safe_insert_user_data(context, key, &tera_value);
        }
    }

    /// Get default Tera template for tabular data
    fn get_default_tabular_template() -> &'static str {
        r#"# Tabular Data Report

**Repository:** {{ repository | default(value="Unknown") }}
**Generated:** {{ timestamp }}

**Statistics:**
- Total Rows: {{ row_count }}
- Columns: {{ column_count | default(value=0) }}

{% if rows and rows | length > 0 %}
## Data
{% for row in rows %}
### Row {{ loop.index }}
{% for value in row.values %}
- Column {{ loop.index }}: {{ value }}
{% endfor %}
{% if row.metadata %}
**Metadata:**
{% for key, value in row.metadata %}
- {{ key }}: {{ value }}
{% endfor %}
{% endif %}
{% endfor %}
{% else %}
*No data available*
{% endif %}

---
*Generated by RepoStats*
"#
    }

    /// Get default Tera template for hierarchical data
    fn get_default_hierarchical_template() -> &'static str {
        r#"# Hierarchical Data Report

**Repository:** {{ repository | default(value="Unknown") }}
**Generated:** {{ timestamp }}

**Statistics:**
- Total Trees: {{ tree_count }}

{% if trees and trees | length > 0 %}
## Data Structure
{% for tree in trees %}
{{ self::render_tree_node(tree=tree, level=1) }}
{% endfor %}
{% else %}
*No data available*
{% endif %}

{% macro render_tree_node(tree, level) %}
{% set heading_level = level + 2 %}
{% if heading_level == 3 %}### {{ tree.key }}{% elif heading_level == 4 %}#### {{ tree.key }}{% elif heading_level == 5 %}##### {{ tree.key }}{% else %}###### {{ tree.key }}{% endif %}

**Value:** {{ tree.value }}

{% if tree.metadata %}
**Metadata:**
{% for key, value in tree.metadata %}
- {{ key }}: {{ value }}
{% endfor %}
{% endif %}

{% if tree.children and tree.children | length > 0 %}
{% for child in tree.children %}
{{ self::render_tree_node(tree=child, level=level + 1) }}
{% endfor %}
{% endif %}
{% endmacro render_tree_node %}

---
*Generated by RepoStats*
"#
    }

    /// Get default Tera template for key-value data
    fn get_default_keyvalue_template() -> &'static str {
        r#"# Key-Value Data Report

**Repository:** {{ repository | default(value="Unknown") }}
**Generated:** {{ timestamp }}

**Statistics:**
- Total Pairs: {{ pair_count }}

{% if pairs and pairs | length > 0 %}
## Data
{% for pair in pairs | sort(attribute="key") %}
- **{{ pair.key }}**: {{ pair.value }}
{% endfor %}
{% else %}
*No data available*
{% endif %}

---
*Generated by RepoStats*
"#
    }

    /// Get default Tera template for raw data
    fn get_default_raw_template() -> &'static str {
        r#"# Raw Data Report

**Repository:** {{ repository | default(value="Unknown") }}
**Generated:** {{ timestamp }}

{% if content_type %}
**Content-Type:** `{{ content_type }}`
{% endif %}

## Content
{% if content and content | length > 100 %}
```
{{ content }}
```
{% else %}
`{{ content | default(value="No content") }}`
{% endif %}

---
*Generated by RepoStats*
"#
    }

    /// Create Tera context from plugin data
    fn create_context(&self, data: &PluginDataExport) -> Context {
        let mut context = Context::new();

        // Add payload-specific data
        match &data.payload {
            DataPayload::Tabular { rows, .. } => self.add_tabular_context(&mut context, rows),
            DataPayload::Hierarchical { roots } => {
                self.add_hierarchical_context(&mut context, roots)
            }
            DataPayload::KeyValue { data } => self.add_keyvalue_context(&mut context, data),
            DataPayload::Raw { data, content_type } => {
                context.insert("content", data.as_ref());
                if let Some(ct) = content_type {
                    context.insert("content_type", ct);
                }
            }
        };

        // Add formatted timestamp
        use std::time::UNIX_EPOCH;
        if let Ok(duration) = data.timestamp.duration_since(UNIX_EPOCH) {
            let secs = duration.as_secs();
            if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0) {
                context.insert("timestamp", &dt.format("%Y-%m-%d %H:%M:%S UTC").to_string());
            }
        }

        // Add all metadata fields with security checks
        for (key, value) in &data.metadata {
            let json_value = serde_json::Value::String(value.clone());
            self.safe_insert_user_data(&mut context, key, &json_value);
        }

        // Add repository location (prefer path over URL if both exist)
        if let Some(path) = data.metadata.get("repository_path") {
            context.insert("repository", path);
        } else if let Some(url) = data.metadata.get("repository_url") {
            context.insert("repository", url);
        }

        context
    }
}

impl OutputFormatter for TemplateFormatter {
    fn format(&self, data: &PluginDataExport, _use_colors: bool) -> FormatResult {
        // Templates don't use terminal colors (they can define their own styling)
        let context = self.create_context(data);

        let template_name = if self.template_name == "default" {
            // Choose appropriate default template based on payload type
            match &data.payload {
                DataPayload::Tabular { .. } => "tabular_default",
                DataPayload::Hierarchical { .. } => "hierarchical_default",
                DataPayload::KeyValue { .. } => "keyvalue_default",
                DataPayload::Raw { .. } => "raw_default",
            }
        } else {
            &self.template_name
        };

        // Validate template exists before attempting to render
        if !self
            .tera
            .get_template_names()
            .any(|name| name == template_name)
        {
            return Err(PluginError::ExecutionError {
                plugin_name: "TemplateFormatter".to_string(),
                operation: "render template".to_string(),
                cause: format!("Template '{}' not found", template_name),
            });
        }

        self.tera.render(template_name, &context).map_err(|e| {
            // Log the full error internally for debugging
            log::debug!("Template rendering failed for '{}': {}", template_name, e);

            // Sanitize the error message to avoid leaking template/internal details
            let err_str = e.to_string();
            let sanitized = if err_str.len() > 120 {
                format!("{}...", &err_str[..120])
            } else {
                err_str
            };

            // Further sanitize by removing potential sensitive patterns
            let sanitized = sanitized
                .replace(&self.template_name, "[template]") // Hide template names
                .replace("Variable `", "Variable [") // Obscure variable references
                .replace("` not found", "] not found");

            PluginError::ExecutionError {
                plugin_name: "TemplateFormatter".to_string(),
                operation: "render template".to_string(),
                cause: format!("Template rendering failed: {}", sanitized),
            }
        })
    }

    fn format_type(&self) -> ExportFormat {
        ExportFormat::Template
    }
}
