//! Template output formatter
//!
//! This provides a basic template system that can be extended with Tera in the future.
//! Currently supports simple variable substitution and basic control structures.

use super::{FormatResult, OutputFormatter};
use crate::plugin::data_export::{DataPayload, ExportFormat, PluginDataExport, Value};
use crate::plugin::error::{PluginError, PluginResult};
use std::collections::HashMap;

/// Default timeout for HTTP requests when loading templates from URLs
pub const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 60;

/// Template formatter implementation
pub struct TemplateFormatter {
    template_content: Option<String>,
}

impl TemplateFormatter {
    /// Create a new Template formatter
    pub fn new() -> Self {
        Self {
            template_content: None,
        }
    }

    /// Create a Template formatter with custom template content
    pub fn with_template(template: String) -> Self {
        Self {
            template_content: Some(template),
        }
    }

    /// Load template from file path
    pub fn from_file(path: &str) -> PluginResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| PluginError::IoError {
            operation: "read template".to_string(),
            path: path.to_string(),
            cause: e.to_string(),
        })?;

        Ok(Self::with_template(content))
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
                cause: format!("Network request failed: {}", e),
            })?;

        // Check if response is successful
        if !response.status().is_success() {
            return Err(PluginError::IoError {
                operation: "fetch template".to_string(),
                path: url.to_string(),
                cause: format!(
                    "HTTP {} - {}",
                    response.status().as_u16(),
                    response
                        .status()
                        .canonical_reason()
                        .unwrap_or("Unknown error")
                ),
            });
        }

        // Get response text
        let content = response.text().await.map_err(|e| PluginError::IoError {
            operation: "read template response".to_string(),
            path: url.to_string(),
            cause: format!("Failed to read response body: {}", e),
        })?;

        Ok(Self::with_template(content))
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

    /// Convert Value to string for template substitution
    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Timestamp(ts) => format!("{:?}", ts),
            Value::Duration(d) => format!("{:?}", d),
            Value::Null => String::new(),
        }
    }

    /// Simple template variable substitution
    /// Replaces {{ variable_name }} with corresponding values
    fn substitute_variables(&self, template: &str, variables: &HashMap<String, String>) -> String {
        let mut result = template.to_string();

        for (key, value) in variables {
            let pattern = format!("{{{{{}}}}}", key);
            result = result.replace(&pattern, value);
        }

        result
    }

    /// Create template variables from tabular data
    fn create_tabular_variables(
        &self,
        rows: &[crate::plugin::data_export::Row],
    ) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        variables.insert("row_count".to_string(), rows.len().to_string());

        if !rows.is_empty() {
            let max_cols = rows.iter().map(|r| r.values.len()).max().unwrap_or(0);
            variables.insert("column_count".to_string(), max_cols.to_string());

            // Create CSV-like representation for template use
            let mut csv_data = String::new();
            for (i, row) in rows.iter().enumerate() {
                for (j, value) in row.values.iter().enumerate() {
                    csv_data.push_str(&self.value_to_string(value));
                    if j < row.values.len() - 1 {
                        csv_data.push(',');
                    }
                }
                if i < rows.len() - 1 {
                    csv_data.push('\n');
                }
            }
            variables.insert("csv_data".to_string(), csv_data);
        }

        variables
    }

    /// Create template variables from hierarchical data
    fn create_hierarchical_variables(
        &self,
        roots: &[crate::plugin::data_export::TreeNode],
    ) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        variables.insert("tree_count".to_string(), roots.len().to_string());

        // Create JSON-like representation
        let mut json_data = String::from("[\n");
        for (i, root) in roots.iter().enumerate() {
            json_data.push_str(&self.tree_to_json_string(root, 1));
            if i < roots.len() - 1 {
                json_data.push(',');
            }
            json_data.push('\n');
        }
        json_data.push(']');

        variables.insert("json_data".to_string(), json_data);
        variables
    }

    /// Convert tree node to JSON-like string representation
    fn tree_to_json_string(
        &self,
        node: &crate::plugin::data_export::TreeNode,
        indent_level: usize,
    ) -> String {
        let indent = "  ".repeat(indent_level);
        let mut json = String::new();

        json.push_str(&format!("{}{{\n", indent));
        json.push_str(&format!(
            "{}  \"key\": \"{}\",\n",
            indent,
            node.key.replace('"', "\\\"")
        ));
        json.push_str(&format!(
            "{}  \"value\": \"{}\",\n",
            indent,
            self.value_to_string(&node.value).replace('"', "\\\"")
        ));

        if !node.children.is_empty() {
            json.push_str(&format!("{}  \"children\": [\n", indent));
            for (i, child) in node.children.iter().enumerate() {
                json.push_str(&self.tree_to_json_string(child, indent_level + 2));
                if i < node.children.len() - 1 {
                    json.push(',');
                }
                json.push('\n');
            }
            json.push_str(&format!("{}  ]\n", indent));
        } else {
            json.push_str(&format!("{}  \"children\": []\n", indent));
        }

        json.push_str(&format!("{}}}", indent));
        json
    }

    /// Create template variables from key-value data
    fn create_keyvalue_variables(&self, data: &HashMap<String, Value>) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        variables.insert("pair_count".to_string(), data.len().to_string());

        // Add each key-value pair as separate variables
        for (key, value) in data {
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
            variables.insert(safe_key, self.value_to_string(value));
        }

        // Create formatted list
        let mut formatted_pairs = String::new();
        let mut sorted_keys: Vec<_> = data.keys().collect();
        sorted_keys.sort();

        for key in sorted_keys {
            if let Some(value) = data.get(key) {
                formatted_pairs.push_str(&format!("{}: {}\n", key, self.value_to_string(value)));
            }
        }

        variables.insert("formatted_pairs".to_string(), formatted_pairs);
        variables
    }

    /// Get default template for a given payload type
    fn get_default_template(&self, payload: &DataPayload) -> &'static str {
        match payload {
            DataPayload::Tabular { .. } => {
                r#"# Tabular Data Report

**Repository:** {{repository}}
**Generated:** {{timestamp}}

**Statistics:**
- Total Rows: {{row_count}}
- Columns: {{column_count}}

## Data (CSV Format)
```
{{csv_data}}
```

Generated by RepoStats
"#
            }
            DataPayload::Hierarchical { .. } => {
                r#"# Hierarchical Data Report

**Repository:** {{repository}}
**Generated:** {{timestamp}}

**Statistics:**
- Total Trees: {{tree_count}}

## Data Structure
```json
{{json_data}}
```

Generated by RepoStats
"#
            }
            DataPayload::KeyValue { .. } => {
                r#"# Key-Value Data Report

**Repository:** {{repository}}
**Generated:** {{timestamp}}

**Statistics:**
- Total Pairs: {{pair_count}}

## Data
{{formatted_pairs}}

Generated by RepoStats
"#
            }
            DataPayload::Raw { .. } => {
                r#"# Raw Data Report

**Repository:** {{repository}}
**Generated:** {{timestamp}}

## Content
{{content}}

Generated by RepoStats
"#
            }
        }
    }

    /// Format template with data
    fn format_with_template(&self, template: &str, data: &PluginDataExport) -> String {
        // Start with payload-specific variables
        let mut variables = match &data.payload {
            DataPayload::Tabular { rows, .. } => self.create_tabular_variables(rows),
            DataPayload::Hierarchical { roots } => self.create_hierarchical_variables(roots),
            DataPayload::KeyValue { data } => self.create_keyvalue_variables(data),
            DataPayload::Raw { data, content_type } => {
                let mut vars = HashMap::new();
                vars.insert("content".to_string(), data.as_ref().clone());
                if let Some(ct) = content_type {
                    vars.insert("content_type".to_string(), ct.clone());
                }
                vars
            }
        };

        // Add formatted timestamp
        use std::time::UNIX_EPOCH;
        if let Ok(duration) = data.timestamp.duration_since(UNIX_EPOCH) {
            let secs = duration.as_secs();
            // Convert to a simple readable format: YYYY-MM-DD HH:MM:SS UTC
            let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0);
            if let Some(dt) = datetime {
                variables.insert(
                    "timestamp".to_string(),
                    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                );
            }
        }

        // Add all metadata fields directly (no prefix)
        for (key, value) in &data.metadata {
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
            variables.insert(safe_key, value.clone());
        }

        // Add repository location (prefer path over URL if both exist)
        if let Some(path) = data.metadata.get("repository_path") {
            variables.insert("repository".to_string(), path.clone());
        } else if let Some(url) = data.metadata.get("repository_url") {
            variables.insert("repository".to_string(), url.clone());
        }

        self.substitute_variables(template, &variables)
    }
}

impl OutputFormatter for TemplateFormatter {
    fn format(&self, data: &PluginDataExport, _use_colors: bool) -> FormatResult {
        // Templates don't use terminal colors (they can define their own styling)

        let template = match &self.template_content {
            Some(custom_template) => custom_template.as_str(),
            None => self.get_default_template(&data.payload),
        };

        Ok(self.format_with_template(template, data))
    }

    fn format_type(&self) -> ExportFormat {
        ExportFormat::Custom("template".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::data_export::{
        ColumnDef, ColumnType, DataPayload, DataSchema, PluginDataExport, Row,
    };
    use std::sync::Arc;

    #[test]
    fn test_variable_substitution() {
        let formatter = TemplateFormatter::new();
        let template = "Hello {{name}}, you have {{count}} messages";
        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "Alice".to_string());
        variables.insert("count".to_string(), "5".to_string());

        let result = formatter.substitute_variables(template, &variables);
        assert_eq!(result, "Hello Alice, you have 5 messages");
    }

    #[test]
    fn test_custom_template() {
        let custom_template = "Row count: {{row_count}}".to_string();
        let formatter = TemplateFormatter::with_template(custom_template);

        use crate::plugin::data_export::ExportHints;
        use std::time::SystemTime;

        let data = PluginDataExport {
            plugin_id: "test".to_string(),
            scan_id: "scan1".to_string(),
            timestamp: SystemTime::now(),
            payload: DataPayload::Tabular {
                rows: Arc::new(vec![Row {
                    values: vec![Value::String("test".to_string())],
                    metadata: HashMap::new(),
                }]),
                schema: Arc::new(DataSchema {
                    name: "test_schema".to_string(),
                    version: "1.0".to_string(),
                    columns: vec![ColumnDef {
                        name: "col_0".to_string(),
                        column_type: ColumnType::String,
                        nullable: false,
                        description: None,
                        default_value: None,
                    }],
                    metadata: HashMap::new(),
                }),
            },
            hints: ExportHints::default(),
            metadata: HashMap::new(),
        };

        let result = formatter.format(&data, false).unwrap();
        assert_eq!(result, "Row count: 1");
    }

    #[test]
    fn test_url_validation() {
        // Test valid HTTPS URL
        let invalid_url = "ftp://example.com/template.txt";
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(TemplateFormatter::from_url(invalid_url));
        assert!(result.is_err());
        if let Err(PluginError::ConfigurationError { message, .. }) = result {
            assert!(message.contains("Invalid URL scheme"));
        } else {
            panic!("Expected ConfigurationError for invalid URL scheme");
        }
    }

    #[test]
    fn test_from_source_url_detection() {
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Test HTTPS URL detection (will fail due to network, but should attempt URL loading)
        let result = rt.block_on(TemplateFormatter::from_source(
            "https://example.com/template.txt",
        ));
        assert!(result.is_err());
        // Should be a network error, not a file error
        if let Err(PluginError::IoError { operation, .. }) = result {
            assert_eq!(operation, "fetch template");
        } else if let Err(PluginError::ConfigurationError { .. }) = result {
            // This is also acceptable if HTTP client creation fails
        } else {
            panic!("Expected IoError or ConfigurationError for URL loading");
        }
    }

    #[test]
    fn test_custom_timeout() {
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Test with short timeout - should still be a network/timeout error but validates timeout parameter
        let result = rt.block_on(TemplateFormatter::from_url_with_timeout(
            "https://example.com/template.txt",
            5,
        ));
        assert!(result.is_err());

        // Test with very short timeout to ensure it's being used
        let result = rt.block_on(TemplateFormatter::from_source_with_timeout(
            "https://httpstat.us/200?sleep=10000",
            1,
        ));
        assert!(result.is_err());
        // Should be a network/timeout error due to the short timeout
        if let Err(PluginError::IoError { cause, .. }) = result {
            // Should contain timeout-related error message
            assert!(
                cause.to_lowercase().contains("timeout")
                    || cause.to_lowercase().contains("time")
                    || cause.contains("Network request failed")
            );
        } else if let Err(PluginError::ConfigurationError { .. }) = result {
            // This is also acceptable if HTTP client creation fails
        } else {
            panic!("Expected timeout-related error for very short timeout");
        }
    }

    #[test]
    fn test_default_timeout_constant() {
        // Verify the constant is reasonable (60 seconds should be sufficient for most connections)
        assert_eq!(DEFAULT_HTTP_TIMEOUT_SECS, 60);
        assert!(DEFAULT_HTTP_TIMEOUT_SECS >= 30); // At least 30 seconds
        assert!(DEFAULT_HTTP_TIMEOUT_SECS <= 120); // No more than 2 minutes
    }
}
