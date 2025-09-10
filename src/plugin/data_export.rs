//! Data export structures for plugin data interchange
//!
//! This module defines the data structures used for exchanging data between
//! plugins, particularly from processing plugins to output plugins.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Types of data export formats supported by the plugin system
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DataExportType {
    /// Tabular data with rows and columns
    Tabular,
    /// Hierarchical data with nested structures
    Hierarchical,
    /// Key-value pairs
    KeyValue,
    /// Raw unstructured data
    Raw,
}

/// Value types that can be stored in data exports
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// String value
    String(String),
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// Timestamp value
    Timestamp(SystemTime),
    /// Duration value
    Duration(Duration),
    /// Null/empty value
    Null,
}

/// Column type definitions for structured data
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnType {
    /// String column
    String,
    /// Integer column
    Integer,
    /// Float column
    Float,
    /// Boolean column
    Boolean,
    /// Timestamp column
    Timestamp,
    /// Duration column
    Duration,
}

/// Column definition with metadata
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnDef {
    /// Column name
    pub name: String,
    /// Column type
    pub column_type: ColumnType,
    /// Whether the column allows null values
    pub nullable: bool,
    /// Optional description
    pub description: Option<String>,
    /// Optional default value
    pub default_value: Option<Value>,
}

impl ColumnDef {
    /// Create a new column definition builder
    pub fn builder(name: impl Into<String>, column_type: ColumnType) -> ColumnDefBuilder {
        ColumnDefBuilder {
            name: name.into(),
            column_type,
            nullable: true,
            description: None,
            default_value: None,
        }
    }

    /// Create a simple column definition
    pub fn new(name: impl Into<String>, column_type: ColumnType) -> Self {
        Self {
            name: name.into(),
            column_type,
            nullable: true,
            description: None,
            default_value: None,
        }
    }
}

/// Builder for ColumnDef
pub struct ColumnDefBuilder {
    name: String,
    column_type: ColumnType,
    nullable: bool,
    description: Option<String>,
    default_value: Option<Value>,
}

impl ColumnDefBuilder {
    /// Set whether the column is nullable
    pub fn nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    /// Set the column description
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the default value
    pub fn default_value(mut self, value: Value) -> Self {
        self.default_value = Some(value);
        self
    }

    /// Build the column definition
    pub fn build(self) -> ColumnDef {
        ColumnDef {
            name: self.name,
            column_type: self.column_type,
            nullable: self.nullable,
            description: self.description,
            default_value: self.default_value,
        }
    }
}

/// Schema definition for structured data
#[derive(Clone, Debug, PartialEq)]
pub struct DataSchema {
    /// Schema name
    pub name: String,
    /// Schema version
    pub version: String,
    /// Column definitions
    pub columns: Vec<ColumnDef>,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

impl DataSchema {
    /// Create a new schema
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            columns: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a column to the schema
    pub fn add_column(mut self, column: ColumnDef) -> Self {
        self.columns.push(column);
        self
    }

    /// Add metadata to the schema
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get a column by name
    pub fn get_column(&self, name: &str) -> Option<&ColumnDef> {
        self.columns.iter().find(|col| col.name == name)
    }

    /// Validate that a value is compatible with a column
    pub fn validate_value(&self, column_name: &str, value: &Value) -> Result<(), String> {
        let column = self
            .get_column(column_name)
            .ok_or_else(|| format!("Column '{}' not found in schema", column_name))?;

        // Check for null values
        if matches!(value, Value::Null) && !column.nullable {
            return Err(format!(
                "Column '{}' does not allow null values",
                column_name
            ));
        }

        // Check type compatibility
        let compatible = match (&column.column_type, value) {
            (ColumnType::String, Value::String(_)) => true,
            (ColumnType::Integer, Value::Integer(_)) => true,
            (ColumnType::Float, Value::Float(_)) => true,
            (ColumnType::Boolean, Value::Boolean(_)) => true,
            (ColumnType::Timestamp, Value::Timestamp(_)) => true,
            (ColumnType::Duration, Value::Duration(_)) => true,
            (_, Value::Null) => column.nullable,
            _ => false,
        };

        if !compatible {
            return Err(format!(
                "Value type mismatch for column '{}': expected {:?}, got {:?}",
                column_name, column.column_type, value
            ));
        }

        Ok(())
    }
}

/// Row structure for tabular data
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Column values in schema order
    pub values: Vec<Value>,
    /// Optional row metadata
    pub metadata: HashMap<String, String>,
}

impl Row {
    /// Create a new row
    pub fn new(values: Vec<Value>) -> Self {
        Self {
            values,
            metadata: HashMap::new(),
        }
    }

    /// Create a row with metadata
    pub fn with_metadata(values: Vec<Value>, metadata: HashMap<String, String>) -> Self {
        Self { values, metadata }
    }

    /// Add metadata to the row
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get value by column index
    pub fn get_value(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    /// Get value by column name (requires schema for lookup)
    pub fn get_value_by_name(&self, schema: &DataSchema, column_name: &str) -> Option<&Value> {
        let column_index = schema
            .columns
            .iter()
            .position(|col| col.name == column_name)?;
        self.get_value(column_index)
    }
}

/// Tree node structure for hierarchical data
#[derive(Clone, Debug, PartialEq)]
pub struct TreeNode {
    /// Node key/identifier
    pub key: String,
    /// Node value
    pub value: Value,
    /// Child nodes
    pub children: Vec<TreeNode>,
    /// Optional node metadata
    pub metadata: HashMap<String, String>,
}

impl TreeNode {
    /// Create a new tree node
    pub fn new(key: impl Into<String>, value: Value) -> Self {
        Self {
            key: key.into(),
            value,
            children: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a child node
    pub fn add_child(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self
    }

    /// Add metadata to the node
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Find a child by key
    pub fn find_child(&self, key: &str) -> Option<&TreeNode> {
        self.children.iter().find(|child| child.key == key)
    }

    /// Find a child by key (mutable)
    pub fn find_child_mut(&mut self, key: &str) -> Option<&mut TreeNode> {
        self.children.iter_mut().find(|child| child.key == key)
    }

    /// Get all leaf nodes (nodes without children)
    pub fn get_leaves(&self) -> Vec<&TreeNode> {
        if self.children.is_empty() {
            vec![self]
        } else {
            self.children
                .iter()
                .flat_map(|child| child.get_leaves())
                .collect()
        }
    }
}

/// Data payload containing the actual exported data
#[derive(Clone, Debug, PartialEq)]
pub enum DataPayload {
    /// Tabular data with rows
    Tabular {
        /// Schema definition
        schema: Arc<DataSchema>,
        /// Data rows
        rows: Arc<Vec<Row>>,
    },
    /// Hierarchical data as a tree
    Hierarchical {
        /// Root nodes of the tree
        roots: Arc<Vec<TreeNode>>,
    },
    /// Key-value pairs
    KeyValue {
        /// Key-value data
        data: Arc<HashMap<String, Value>>,
    },
    /// Raw unstructured data
    Raw {
        /// Raw data as string
        data: Arc<String>,
        /// Optional content type hint
        content_type: Option<String>,
    },
}

impl DataPayload {
    /// Create tabular data payload
    pub fn tabular(schema: DataSchema, rows: Vec<Row>) -> Self {
        Self::Tabular {
            schema: Arc::new(schema),
            rows: Arc::new(rows),
        }
    }

    /// Create hierarchical data payload
    pub fn hierarchical(roots: Vec<TreeNode>) -> Self {
        Self::Hierarchical {
            roots: Arc::new(roots),
        }
    }

    /// Create key-value data payload
    pub fn key_value(data: HashMap<String, Value>) -> Self {
        Self::KeyValue {
            data: Arc::new(data),
        }
    }

    /// Create raw data payload
    pub fn raw(data: String, content_type: Option<String>) -> Self {
        Self::Raw {
            data: Arc::new(data),
            content_type,
        }
    }

    /// Get the data export type
    pub fn export_type(&self) -> DataExportType {
        match self {
            Self::Tabular { .. } => DataExportType::Tabular,
            Self::Hierarchical { .. } => DataExportType::Hierarchical,
            Self::KeyValue { .. } => DataExportType::KeyValue,
            Self::Raw { .. } => DataExportType::Raw,
        }
    }

    /// Get estimated memory usage
    pub fn estimated_size(&self) -> usize {
        match self {
            Self::Tabular { schema, rows } => {
                std::mem::size_of_val(schema.as_ref())
                    + rows
                        .iter()
                        .map(|row| {
                            row.values.len() * std::mem::size_of::<Value>()
                                + row.metadata.len() * 64 // rough estimate for HashMap entries
                        })
                        .sum::<usize>()
            }
            Self::Hierarchical { roots } => {
                fn node_size(node: &TreeNode) -> usize {
                    std::mem::size_of_val(node) + node.children.iter().map(node_size).sum::<usize>()
                }
                roots.iter().map(node_size).sum()
            }
            Self::KeyValue { data } => {
                data.len() * (64 + std::mem::size_of::<Value>()) // rough estimate
            }
            Self::Raw { data, .. } => data.len(),
        }
    }
}

/// Export format types
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// Console/terminal output with color support
    Console,
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
    /// Custom format with identifier
    Custom(String),
}

impl ExportFormat {
    /// Get file extension for this format
    pub fn file_extension(&self) -> Option<&'static str> {
        match self {
            Self::Console => None,
            Self::Json => Some("json"),
            Self::Csv => Some("csv"),
            Self::Tsv => Some("tsv"),
            Self::Xml => Some("xml"),
            Self::Html => Some("html"),
            Self::Markdown => Some("md"),
            Self::Yaml => Some("yaml"),
            Self::Template => Some("j2"),
            Self::Custom(_) => None,
        }
    }

    /// Detect format from file extension
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "csv" => Some(Self::Csv),
            "tsv" => Some(Self::Tsv),
            "xml" => Some(Self::Xml),
            "html" | "htm" => Some(Self::Html),
            "md" | "markdown" => Some(Self::Markdown),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }

    /// Get MIME type for this format
    pub fn mime_type(&self) -> Option<&'static str> {
        match self {
            Self::Console => None,
            Self::Json => Some("application/json"),
            Self::Csv => Some("text/csv"),
            Self::Tsv => Some("text/tab-separated-values"),
            Self::Xml => Some("application/xml"),
            Self::Html => Some("text/html"),
            Self::Markdown => Some("text/markdown"),
            Self::Yaml => Some("application/x-yaml"),
            Self::Template => Some("text/plain"),
            Self::Custom(_) => None,
        }
    }
}

/// Export hints providing formatting preferences
#[derive(Clone, Debug, PartialEq)]
pub struct ExportHints {
    /// Preferred output format
    pub format: ExportFormat,
    /// Maximum number of rows to export (None for unlimited)
    pub max_rows: Option<usize>,
    /// Whether to include column headers
    pub include_headers: bool,
    /// Whether to pretty print (if applicable)
    pub pretty_print: bool,
    /// Custom formatting options
    pub custom_options: HashMap<String, String>,
}

impl ExportHints {
    /// Create new export hints with default values
    pub fn new(format: ExportFormat) -> Self {
        Self {
            format,
            max_rows: None,
            include_headers: true,
            pretty_print: true,
            custom_options: HashMap::new(),
        }
    }

    /// Set maximum rows
    pub fn max_rows(mut self, max_rows: usize) -> Self {
        self.max_rows = Some(max_rows);
        self
    }

    /// Set whether to include headers
    pub fn include_headers(mut self, include_headers: bool) -> Self {
        self.include_headers = include_headers;
        self
    }

    /// Set pretty print option
    pub fn pretty_print(mut self, pretty_print: bool) -> Self {
        self.pretty_print = pretty_print;
        self
    }

    /// Add custom option
    pub fn custom_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_options.insert(key.into(), value.into());
        self
    }

    /// Get custom option value
    pub fn get_custom_option(&self, key: &str) -> Option<&String> {
        self.custom_options.get(key)
    }
}

impl Default for ExportHints {
    fn default() -> Self {
        Self::new(ExportFormat::Console)
    }
}

/// Main plugin data export structure
#[derive(Clone, Debug, PartialEq)]
pub struct PluginDataExport {
    /// Plugin identifier
    pub plugin_id: String,
    /// Scan identifier
    pub scan_id: String,
    /// Export timestamp
    pub timestamp: SystemTime,
    /// Data payload
    pub payload: DataPayload,
    /// Export hints
    pub hints: ExportHints,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl PluginDataExport {
    /// Create a new plugin data export
    pub fn new(
        plugin_id: impl Into<String>,
        scan_id: impl Into<String>,
        payload: DataPayload,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            scan_id: scan_id.into(),
            timestamp: SystemTime::now(),
            payload,
            hints: ExportHints::default(),
            metadata: HashMap::new(),
        }
    }

    /// Set export hints
    pub fn with_hints(mut self, hints: ExportHints) -> Self {
        self.hints = hints;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get the data export type
    pub fn export_type(&self) -> DataExportType {
        self.payload.export_type()
    }

    /// Get estimated memory usage
    pub fn estimated_size(&self) -> usize {
        self.payload.estimated_size()
            + self.plugin_id.len()
            + self.scan_id.len()
            + self.metadata.len() * 64 // rough estimate for metadata
    }

    /// Create a builder for this export
    pub fn builder(
        plugin_id: impl Into<String>,
        scan_id: impl Into<String>,
    ) -> PluginDataExportBuilder {
        PluginDataExportBuilder::new(plugin_id, scan_id)
    }
}

/// Builder for PluginDataExport
pub struct PluginDataExportBuilder {
    plugin_id: String,
    scan_id: String,
    timestamp: SystemTime,
    payload: Option<DataPayload>,
    hints: ExportHints,
    metadata: HashMap<String, String>,
}

impl PluginDataExportBuilder {
    /// Create a new builder
    pub fn new(plugin_id: impl Into<String>, scan_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            scan_id: scan_id.into(),
            timestamp: SystemTime::now(),
            payload: None,
            hints: ExportHints::default(),
            metadata: HashMap::new(),
        }
    }

    /// Set the payload
    pub fn payload(mut self, payload: DataPayload) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Set export hints
    pub fn hints(mut self, hints: ExportHints) -> Self {
        self.hints = hints;
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set custom timestamp
    pub fn timestamp(mut self, timestamp: SystemTime) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Build the PluginDataExport
    pub fn build(self) -> Result<PluginDataExport, String> {
        let payload = self.payload.ok_or("Payload is required")?;

        Ok(PluginDataExport {
            plugin_id: self.plugin_id,
            scan_id: self.scan_id,
            timestamp: self.timestamp,
            payload,
            hints: self.hints,
            metadata: self.metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_export_type_variants_exist() {
        // Test all expected variants exist
        let tabular = DataExportType::Tabular;
        let hierarchical = DataExportType::Hierarchical;
        let key_value = DataExportType::KeyValue;
        let raw = DataExportType::Raw;

        assert_eq!(tabular, DataExportType::Tabular);
        assert_eq!(hierarchical, DataExportType::Hierarchical);
        assert_eq!(key_value, DataExportType::KeyValue);
        assert_eq!(raw, DataExportType::Raw);
    }

    #[test]
    fn test_data_export_type_equality() {
        // Test equality comparison
        assert_eq!(DataExportType::Tabular, DataExportType::Tabular);
        assert_ne!(DataExportType::Tabular, DataExportType::Hierarchical);
        assert_ne!(DataExportType::KeyValue, DataExportType::Raw);
    }

    #[test]
    fn test_data_export_type_hash() {
        // Test that types can be used as HashMap keys
        let mut map = HashMap::new();
        map.insert(DataExportType::Tabular, "table data");
        map.insert(DataExportType::Hierarchical, "tree data");
        map.insert(DataExportType::KeyValue, "kv data");
        map.insert(DataExportType::Raw, "raw data");

        assert_eq!(map.get(&DataExportType::Tabular), Some(&"table data"));
        assert_eq!(map.get(&DataExportType::Hierarchical), Some(&"tree data"));
        assert_eq!(map.get(&DataExportType::KeyValue), Some(&"kv data"));
        assert_eq!(map.get(&DataExportType::Raw), Some(&"raw data"));
    }

    #[test]
    fn test_data_export_type_clone() {
        let original = DataExportType::Tabular;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_data_export_type_debug() {
        let tabular = DataExportType::Tabular;
        let debug_str = format!("{:?}", tabular);
        assert_eq!(debug_str, "Tabular");
    }

    #[test]
    fn test_value_variants_exist() {
        // Test all expected Value variants exist
        let string_val = Value::String("test".to_string());
        let int_val = Value::Integer(42);
        let float_val = Value::Float(3.14);
        let bool_val = Value::Boolean(true);
        let timestamp_val = Value::Timestamp(SystemTime::now());
        let duration_val = Value::Duration(Duration::from_secs(60));
        let null_val = Value::Null;

        // Test pattern matching works
        match string_val {
            Value::String(_) => {}
            _ => panic!("Expected String variant"),
        }
        match int_val {
            Value::Integer(_) => {}
            _ => panic!("Expected Integer variant"),
        }
        match float_val {
            Value::Float(_) => {}
            _ => panic!("Expected Float variant"),
        }
        match bool_val {
            Value::Boolean(_) => {}
            _ => panic!("Expected Boolean variant"),
        }
        match timestamp_val {
            Value::Timestamp(_) => {}
            _ => panic!("Expected Timestamp variant"),
        }
        match duration_val {
            Value::Duration(_) => {}
            _ => panic!("Expected Duration variant"),
        }
        match null_val {
            Value::Null => {}
            _ => panic!("Expected Null variant"),
        }
    }

    #[test]
    fn test_value_equality() {
        assert_eq!(
            Value::String("test".to_string()),
            Value::String("test".to_string())
        );
        assert_eq!(Value::Integer(42), Value::Integer(42));
        assert_eq!(Value::Float(3.14), Value::Float(3.14));
        assert_eq!(Value::Boolean(true), Value::Boolean(true));
        assert_eq!(Value::Null, Value::Null);

        // Test inequality
        assert_ne!(
            Value::String("test".to_string()),
            Value::String("other".to_string())
        );
        assert_ne!(Value::Integer(42), Value::Integer(43));
        assert_ne!(Value::Boolean(true), Value::Boolean(false));
        assert_ne!(Value::Null, Value::String("".to_string()));
    }

    #[test]
    fn test_value_clone() {
        let original = Value::String("test".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);

        let int_original = Value::Integer(42);
        let int_cloned = int_original.clone();
        assert_eq!(int_original, int_cloned);
    }

    #[test]
    fn test_column_type_variants_exist() {
        // Test all expected ColumnType variants exist
        let string_col = ColumnType::String;
        let int_col = ColumnType::Integer;
        let float_col = ColumnType::Float;
        let bool_col = ColumnType::Boolean;
        let timestamp_col = ColumnType::Timestamp;
        let duration_col = ColumnType::Duration;

        assert_eq!(string_col, ColumnType::String);
        assert_eq!(int_col, ColumnType::Integer);
        assert_eq!(float_col, ColumnType::Float);
        assert_eq!(bool_col, ColumnType::Boolean);
        assert_eq!(timestamp_col, ColumnType::Timestamp);
        assert_eq!(duration_col, ColumnType::Duration);
    }

    #[test]
    fn test_column_type_equality() {
        assert_eq!(ColumnType::String, ColumnType::String);
        assert_ne!(ColumnType::String, ColumnType::Integer);
        assert_ne!(ColumnType::Float, ColumnType::Boolean);
    }

    #[test]
    fn test_column_type_clone() {
        let original = ColumnType::String;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_column_def_new() {
        let col = ColumnDef::new("test_col", ColumnType::String);
        assert_eq!(col.name, "test_col");
        assert_eq!(col.column_type, ColumnType::String);
        assert_eq!(col.nullable, true);
        assert_eq!(col.description, None);
        assert_eq!(col.default_value, None);
    }

    #[test]
    fn test_column_def_builder_basic() {
        let col = ColumnDef::builder("test_col", ColumnType::Integer).build();
        assert_eq!(col.name, "test_col");
        assert_eq!(col.column_type, ColumnType::Integer);
        assert_eq!(col.nullable, true);
        assert_eq!(col.description, None);
        assert_eq!(col.default_value, None);
    }

    #[test]
    fn test_column_def_builder_with_options() {
        let col = ColumnDef::builder("test_col", ColumnType::String)
            .nullable(false)
            .description("A test column")
            .default_value(Value::String("default".to_string()))
            .build();

        assert_eq!(col.name, "test_col");
        assert_eq!(col.column_type, ColumnType::String);
        assert_eq!(col.nullable, false);
        assert_eq!(col.description, Some("A test column".to_string()));
        assert_eq!(
            col.default_value,
            Some(Value::String("default".to_string()))
        );
    }

    #[test]
    fn test_column_def_builder_fluent_interface() {
        let col = ColumnDef::builder("id", ColumnType::Integer)
            .nullable(false)
            .description("Primary key")
            .build();

        assert_eq!(col.name, "id");
        assert_eq!(col.column_type, ColumnType::Integer);
        assert_eq!(col.nullable, false);
        assert_eq!(col.description, Some("Primary key".to_string()));
    }

    #[test]
    fn test_column_def_equality() {
        let col1 = ColumnDef::new("test", ColumnType::String);
        let col2 = ColumnDef::new("test", ColumnType::String);
        let col3 = ColumnDef::new("other", ColumnType::String);

        assert_eq!(col1, col2);
        assert_ne!(col1, col3);
    }

    #[test]
    fn test_column_def_clone() {
        let original = ColumnDef::builder("test", ColumnType::Boolean)
            .description("Test column")
            .build();
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn test_data_schema_new() {
        let schema = DataSchema::new("test_schema", "1.0");
        assert_eq!(schema.name, "test_schema");
        assert_eq!(schema.version, "1.0");
        assert!(schema.columns.is_empty());
        assert!(schema.metadata.is_empty());
    }

    #[test]
    fn test_data_schema_add_column() {
        let col = ColumnDef::new("test_col", ColumnType::String);
        let schema = DataSchema::new("test_schema", "1.0").add_column(col);

        assert_eq!(schema.columns.len(), 1);
        assert_eq!(schema.columns[0].name, "test_col");
        assert_eq!(schema.columns[0].column_type, ColumnType::String);
    }

    #[test]
    fn test_data_schema_add_multiple_columns() {
        let col1 = ColumnDef::new("id", ColumnType::Integer);
        let col2 = ColumnDef::new("name", ColumnType::String);
        let col3 = ColumnDef::new("active", ColumnType::Boolean);

        let schema = DataSchema::new("users", "1.0")
            .add_column(col1)
            .add_column(col2)
            .add_column(col3);

        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.columns[0].name, "id");
        assert_eq!(schema.columns[1].name, "name");
        assert_eq!(schema.columns[2].name, "active");
    }

    #[test]
    fn test_data_schema_add_metadata() {
        let schema = DataSchema::new("test_schema", "1.0")
            .add_metadata("author", "test")
            .add_metadata("description", "A test schema");

        assert_eq!(schema.metadata.len(), 2);
        assert_eq!(schema.metadata.get("author"), Some(&"test".to_string()));
        assert_eq!(
            schema.metadata.get("description"),
            Some(&"A test schema".to_string())
        );
    }

    #[test]
    fn test_data_schema_get_column() {
        let col1 = ColumnDef::new("id", ColumnType::Integer);
        let col2 = ColumnDef::new("name", ColumnType::String);

        let schema = DataSchema::new("test", "1.0")
            .add_column(col1)
            .add_column(col2);

        let found_col = schema.get_column("name");
        assert!(found_col.is_some());
        assert_eq!(found_col.unwrap().name, "name");
        assert_eq!(found_col.unwrap().column_type, ColumnType::String);

        let not_found = schema.get_column("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_data_schema_validate_value_success() {
        let col = ColumnDef::new("test_col", ColumnType::String);
        let schema = DataSchema::new("test", "1.0").add_column(col);

        let result = schema.validate_value("test_col", &Value::String("test".to_string()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_data_schema_validate_value_type_mismatch() {
        let col = ColumnDef::new("test_col", ColumnType::String);
        let schema = DataSchema::new("test", "1.0").add_column(col);

        let result = schema.validate_value("test_col", &Value::Integer(42));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Value type mismatch"));
    }

    #[test]
    fn test_data_schema_validate_value_column_not_found() {
        let schema = DataSchema::new("test", "1.0");

        let result = schema.validate_value("nonexistent", &Value::String("test".to_string()));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Column 'nonexistent' not found"));
    }

    #[test]
    fn test_data_schema_validate_value_null_allowed() {
        let col = ColumnDef::builder("test_col", ColumnType::String)
            .nullable(true)
            .build();
        let schema = DataSchema::new("test", "1.0").add_column(col);

        let result = schema.validate_value("test_col", &Value::Null);
        assert!(result.is_ok());
    }

    #[test]
    fn test_data_schema_validate_value_null_not_allowed() {
        let col = ColumnDef::builder("test_col", ColumnType::String)
            .nullable(false)
            .build();
        let schema = DataSchema::new("test", "1.0").add_column(col);

        let result = schema.validate_value("test_col", &Value::Null);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not allow null values"));
    }

    #[test]
    fn test_data_schema_validate_all_types() {
        let schema = DataSchema::new("test", "1.0")
            .add_column(ColumnDef::new("str_col", ColumnType::String))
            .add_column(ColumnDef::new("int_col", ColumnType::Integer))
            .add_column(ColumnDef::new("float_col", ColumnType::Float))
            .add_column(ColumnDef::new("bool_col", ColumnType::Boolean))
            .add_column(ColumnDef::new("time_col", ColumnType::Timestamp))
            .add_column(ColumnDef::new("dur_col", ColumnType::Duration));

        assert!(schema
            .validate_value("str_col", &Value::String("test".to_string()))
            .is_ok());
        assert!(schema
            .validate_value("int_col", &Value::Integer(42))
            .is_ok());
        assert!(schema
            .validate_value("float_col", &Value::Float(3.14))
            .is_ok());
        assert!(schema
            .validate_value("bool_col", &Value::Boolean(true))
            .is_ok());
        assert!(schema
            .validate_value("time_col", &Value::Timestamp(SystemTime::now()))
            .is_ok());
        assert!(schema
            .validate_value("dur_col", &Value::Duration(Duration::from_secs(60)))
            .is_ok());
    }

    #[test]
    fn test_data_schema_equality() {
        let col1 = ColumnDef::new("id", ColumnType::Integer);
        let col2 = ColumnDef::new("name", ColumnType::String);

        let schema1 = DataSchema::new("test", "1.0")
            .add_column(col1.clone())
            .add_column(col2.clone());

        let schema2 = DataSchema::new("test", "1.0")
            .add_column(col1)
            .add_column(col2);

        let schema3 = DataSchema::new("different", "1.0");

        assert_eq!(schema1, schema2);
        assert_ne!(schema1, schema3);
    }

    #[test]
    fn test_data_schema_clone() {
        let col = ColumnDef::new("test", ColumnType::String);
        let original = DataSchema::new("test", "1.0")
            .add_column(col)
            .add_metadata("key", "value");

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // Row tests
    #[test]
    fn test_row_new() {
        let values = vec![
            Value::Integer(1),
            Value::String("test".to_string()),
            Value::Boolean(true),
        ];
        let row = Row::new(values.clone());

        assert_eq!(row.values, values);
        assert!(row.metadata.is_empty());
    }

    #[test]
    fn test_row_with_metadata() {
        let values = vec![Value::Integer(1)];
        let mut metadata = HashMap::new();
        metadata.insert("id".to_string(), "row1".to_string());

        let row = Row::with_metadata(values.clone(), metadata.clone());

        assert_eq!(row.values, values);
        assert_eq!(row.metadata, metadata);
    }

    #[test]
    fn test_row_add_metadata() {
        let values = vec![Value::String("test".to_string())];
        let row = Row::new(values.clone())
            .add_metadata("id", "row1")
            .add_metadata("source", "test");

        assert_eq!(row.values, values);
        assert_eq!(row.metadata.get("id"), Some(&"row1".to_string()));
        assert_eq!(row.metadata.get("source"), Some(&"test".to_string()));
    }

    #[test]
    fn test_row_get_value() {
        let values = vec![
            Value::Integer(42),
            Value::String("hello".to_string()),
            Value::Boolean(false),
        ];
        let row = Row::new(values);

        assert_eq!(row.get_value(0), Some(&Value::Integer(42)));
        assert_eq!(row.get_value(1), Some(&Value::String("hello".to_string())));
        assert_eq!(row.get_value(2), Some(&Value::Boolean(false)));
        assert_eq!(row.get_value(3), None);
    }

    #[test]
    fn test_row_get_value_by_name() {
        let schema = DataSchema::new("test", "1.0")
            .add_column(ColumnDef::new("id", ColumnType::Integer))
            .add_column(ColumnDef::new("name", ColumnType::String))
            .add_column(ColumnDef::new("active", ColumnType::Boolean));

        let values = vec![
            Value::Integer(1),
            Value::String("test".to_string()),
            Value::Boolean(true),
        ];
        let row = Row::new(values);

        assert_eq!(
            row.get_value_by_name(&schema, "id"),
            Some(&Value::Integer(1))
        );
        assert_eq!(
            row.get_value_by_name(&schema, "name"),
            Some(&Value::String("test".to_string()))
        );
        assert_eq!(
            row.get_value_by_name(&schema, "active"),
            Some(&Value::Boolean(true))
        );
        assert_eq!(row.get_value_by_name(&schema, "nonexistent"), None);
    }

    #[test]
    fn test_row_equality() {
        let values = vec![Value::Integer(1), Value::String("test".to_string())];
        let row1 = Row::new(values.clone());
        let row2 = Row::new(values);
        let row3 = Row::new(vec![Value::Integer(2)]);

        assert_eq!(row1, row2);
        assert_ne!(row1, row3);
    }

    #[test]
    fn test_row_clone() {
        let values = vec![Value::String("test".to_string())];
        let original = Row::new(values).add_metadata("id", "test");
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    // TreeNode tests
    #[test]
    fn test_tree_node_new() {
        let node = TreeNode::new("root", Value::String("root_value".to_string()));

        assert_eq!(node.key, "root");
        assert_eq!(node.value, Value::String("root_value".to_string()));
        assert!(node.children.is_empty());
        assert!(node.metadata.is_empty());
    }

    #[test]
    fn test_tree_node_add_child() {
        let child = TreeNode::new("child", Value::Integer(42));
        let parent =
            TreeNode::new("parent", Value::String("parent_value".to_string())).add_child(child);

        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].key, "child");
        assert_eq!(parent.children[0].value, Value::Integer(42));
    }

    #[test]
    fn test_tree_node_add_multiple_children() {
        let child1 = TreeNode::new("child1", Value::Integer(1));
        let child2 = TreeNode::new("child2", Value::Integer(2));
        let child3 = TreeNode::new("child3", Value::Integer(3));

        let parent = TreeNode::new("parent", Value::String("root".to_string()))
            .add_child(child1)
            .add_child(child2)
            .add_child(child3);

        assert_eq!(parent.children.len(), 3);
        assert_eq!(parent.children[0].key, "child1");
        assert_eq!(parent.children[1].key, "child2");
        assert_eq!(parent.children[2].key, "child3");
    }

    #[test]
    fn test_tree_node_add_metadata() {
        let node = TreeNode::new("test", Value::Integer(42))
            .add_metadata("type", "test_node")
            .add_metadata("level", "1");

        assert_eq!(node.metadata.get("type"), Some(&"test_node".to_string()));
        assert_eq!(node.metadata.get("level"), Some(&"1".to_string()));
    }

    #[test]
    fn test_tree_node_find_child() {
        let child1 = TreeNode::new("child1", Value::Integer(1));
        let child2 = TreeNode::new("child2", Value::Integer(2));

        let parent = TreeNode::new("parent", Value::String("root".to_string()))
            .add_child(child1)
            .add_child(child2);

        let found = parent.find_child("child2");
        assert!(found.is_some());
        assert_eq!(found.unwrap().key, "child2");
        assert_eq!(found.unwrap().value, Value::Integer(2));

        let not_found = parent.find_child("child3");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_tree_node_find_child_mut() {
        let child1 = TreeNode::new("child1", Value::Integer(1));
        let child2 = TreeNode::new("child2", Value::Integer(2));

        let mut parent = TreeNode::new("parent", Value::String("root".to_string()))
            .add_child(child1)
            .add_child(child2);

        let found = parent.find_child_mut("child1");
        assert!(found.is_some());

        if let Some(child) = found {
            child.value = Value::Integer(100);
        }

        assert_eq!(parent.children[0].value, Value::Integer(100));
    }

    #[test]
    fn test_tree_node_get_leaves() {
        // Single node (leaf)
        let leaf = TreeNode::new("leaf", Value::Integer(42));
        let leaves = leaf.get_leaves();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].key, "leaf");

        // Tree with children
        let leaf1 = TreeNode::new("leaf1", Value::Integer(1));
        let leaf2 = TreeNode::new("leaf2", Value::Integer(2));
        let leaf3 = TreeNode::new("leaf3", Value::Integer(3));

        let branch = TreeNode::new("branch", Value::String("branch".to_string()))
            .add_child(leaf2)
            .add_child(leaf3);

        let root = TreeNode::new("root", Value::String("root".to_string()))
            .add_child(leaf1)
            .add_child(branch);

        let leaves = root.get_leaves();
        assert_eq!(leaves.len(), 3);

        let leaf_keys: Vec<&str> = leaves.iter().map(|node| node.key.as_str()).collect();
        assert!(leaf_keys.contains(&"leaf1"));
        assert!(leaf_keys.contains(&"leaf2"));
        assert!(leaf_keys.contains(&"leaf3"));
    }

    #[test]
    fn test_tree_node_equality() {
        let node1 = TreeNode::new("test", Value::Integer(42));
        let node2 = TreeNode::new("test", Value::Integer(42));
        let node3 = TreeNode::new("different", Value::Integer(42));

        assert_eq!(node1, node2);
        assert_ne!(node1, node3);
    }

    #[test]
    fn test_tree_node_clone() {
        let child = TreeNode::new("child", Value::Integer(1));
        let original = TreeNode::new("parent", Value::String("test".to_string()))
            .add_child(child)
            .add_metadata("type", "test");

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // DataPayload tests
    #[test]
    fn test_data_payload_tabular() {
        let schema = DataSchema::new("test", "1.0")
            .add_column(ColumnDef::new("id", ColumnType::Integer))
            .add_column(ColumnDef::new("name", ColumnType::String));

        let rows = vec![
            Row::new(vec![Value::Integer(1), Value::String("Alice".to_string())]),
            Row::new(vec![Value::Integer(2), Value::String("Bob".to_string())]),
        ];

        let payload = DataPayload::tabular(schema, rows);

        assert_eq!(payload.export_type(), DataExportType::Tabular);

        if let DataPayload::Tabular { schema, rows } = payload {
            assert_eq!(schema.name, "test");
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].values[0], Value::Integer(1));
            assert_eq!(rows[1].values[1], Value::String("Bob".to_string()));
        } else {
            panic!("Expected Tabular payload");
        }
    }

    #[test]
    fn test_data_payload_hierarchical() {
        let child1 = TreeNode::new("child1", Value::Integer(1));
        let child2 = TreeNode::new("child2", Value::Integer(2));
        let root = TreeNode::new("root", Value::String("root".to_string()))
            .add_child(child1)
            .add_child(child2);

        let payload = DataPayload::hierarchical(vec![root]);

        assert_eq!(payload.export_type(), DataExportType::Hierarchical);

        if let DataPayload::Hierarchical { roots } = payload {
            assert_eq!(roots.len(), 1);
            assert_eq!(roots[0].key, "root");
            assert_eq!(roots[0].children.len(), 2);
        } else {
            panic!("Expected Hierarchical payload");
        }
    }

    #[test]
    fn test_data_payload_key_value() {
        let mut data = HashMap::new();
        data.insert("name".to_string(), Value::String("test".to_string()));
        data.insert("count".to_string(), Value::Integer(42));
        data.insert("active".to_string(), Value::Boolean(true));

        let payload = DataPayload::key_value(data);

        assert_eq!(payload.export_type(), DataExportType::KeyValue);

        if let DataPayload::KeyValue { data } = payload {
            assert_eq!(data.len(), 3);
            assert_eq!(data.get("name"), Some(&Value::String("test".to_string())));
            assert_eq!(data.get("count"), Some(&Value::Integer(42)));
            assert_eq!(data.get("active"), Some(&Value::Boolean(true)));
        } else {
            panic!("Expected KeyValue payload");
        }
    }

    #[test]
    fn test_data_payload_raw() {
        let data = "Some raw text data".to_string();
        let content_type = Some("text/plain".to_string());

        let payload = DataPayload::raw(data.clone(), content_type.clone());

        assert_eq!(payload.export_type(), DataExportType::Raw);

        if let DataPayload::Raw {
            data: payload_data,
            content_type: payload_content_type,
        } = payload
        {
            assert_eq!(*payload_data, data);
            assert_eq!(payload_content_type, content_type);
        } else {
            panic!("Expected Raw payload");
        }
    }

    #[test]
    fn test_data_payload_raw_without_content_type() {
        let data = "Raw data without content type".to_string();
        let payload = DataPayload::raw(data.clone(), None);

        if let DataPayload::Raw {
            data: payload_data,
            content_type,
        } = payload
        {
            assert_eq!(*payload_data, data);
            assert_eq!(content_type, None);
        } else {
            panic!("Expected Raw payload");
        }
    }

    #[test]
    fn test_data_payload_estimated_size() {
        // Test tabular payload size
        let schema = DataSchema::new("test", "1.0");
        let rows = vec![Row::new(vec![Value::Integer(1)])];
        let tabular_payload = DataPayload::tabular(schema, rows);
        let tabular_size = tabular_payload.estimated_size();
        assert!(tabular_size > 0);

        // Test hierarchical payload size
        let root = TreeNode::new("root", Value::String("test".to_string()));
        let hierarchical_payload = DataPayload::hierarchical(vec![root]);
        let hierarchical_size = hierarchical_payload.estimated_size();
        assert!(hierarchical_size > 0);

        // Test key-value payload size
        let mut data = HashMap::new();
        data.insert("key".to_string(), Value::String("value".to_string()));
        let kv_payload = DataPayload::key_value(data);
        let kv_size = kv_payload.estimated_size();
        assert!(kv_size > 0);

        // Test raw payload size
        let raw_data = "test data".to_string();
        let raw_payload = DataPayload::raw(raw_data.clone(), None);
        let raw_size = raw_payload.estimated_size();
        assert_eq!(raw_size, raw_data.len());
    }

    #[test]
    fn test_data_payload_equality() {
        let schema = DataSchema::new("test", "1.0");
        let rows = vec![Row::new(vec![Value::Integer(1)])];

        let payload1 = DataPayload::tabular(schema.clone(), rows.clone());
        let payload2 = DataPayload::tabular(schema, rows);
        let payload3 = DataPayload::raw("different".to_string(), None);

        assert_eq!(payload1, payload2);
        assert_ne!(payload1, payload3);
    }

    #[test]
    fn test_data_payload_clone() {
        let schema = DataSchema::new("test", "1.0");
        let rows = vec![Row::new(vec![Value::String("test".to_string())])];
        let original = DataPayload::tabular(schema, rows);

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // ExportFormat tests
    #[test]
    fn test_export_format_variants_exist() {
        // Test all expected variants exist
        let console = ExportFormat::Console;
        let json = ExportFormat::Json;
        let csv = ExportFormat::Csv;
        let tsv = ExportFormat::Tsv;
        let xml = ExportFormat::Xml;
        let html = ExportFormat::Html;
        let markdown = ExportFormat::Markdown;
        let yaml = ExportFormat::Yaml;
        let custom = ExportFormat::Custom("custom".to_string());

        assert_eq!(console, ExportFormat::Console);
        assert_eq!(json, ExportFormat::Json);
        assert_eq!(csv, ExportFormat::Csv);
        assert_eq!(tsv, ExportFormat::Tsv);
        assert_eq!(xml, ExportFormat::Xml);
        assert_eq!(html, ExportFormat::Html);
        assert_eq!(markdown, ExportFormat::Markdown);
        assert_eq!(yaml, ExportFormat::Yaml);
        assert_eq!(custom, ExportFormat::Custom("custom".to_string()));
    }

    #[test]
    fn test_export_format_file_extension() {
        assert_eq!(ExportFormat::Console.file_extension(), None);
        assert_eq!(ExportFormat::Json.file_extension(), Some("json"));
        assert_eq!(ExportFormat::Csv.file_extension(), Some("csv"));
        assert_eq!(ExportFormat::Tsv.file_extension(), Some("tsv"));
        assert_eq!(ExportFormat::Xml.file_extension(), Some("xml"));
        assert_eq!(ExportFormat::Html.file_extension(), Some("html"));
        assert_eq!(ExportFormat::Markdown.file_extension(), Some("md"));
        assert_eq!(ExportFormat::Yaml.file_extension(), Some("yaml"));
        assert_eq!(
            ExportFormat::Custom("test".to_string()).file_extension(),
            None
        );
    }

    #[test]
    fn test_export_format_from_extension() {
        assert_eq!(
            ExportFormat::from_extension("json"),
            Some(ExportFormat::Json)
        );
        assert_eq!(
            ExportFormat::from_extension("JSON"),
            Some(ExportFormat::Json)
        );
        assert_eq!(ExportFormat::from_extension("csv"), Some(ExportFormat::Csv));
        assert_eq!(ExportFormat::from_extension("tsv"), Some(ExportFormat::Tsv));
        assert_eq!(ExportFormat::from_extension("xml"), Some(ExportFormat::Xml));
        assert_eq!(
            ExportFormat::from_extension("html"),
            Some(ExportFormat::Html)
        );
        assert_eq!(
            ExportFormat::from_extension("htm"),
            Some(ExportFormat::Html)
        );
        assert_eq!(
            ExportFormat::from_extension("md"),
            Some(ExportFormat::Markdown)
        );
        assert_eq!(
            ExportFormat::from_extension("markdown"),
            Some(ExportFormat::Markdown)
        );
        assert_eq!(
            ExportFormat::from_extension("yaml"),
            Some(ExportFormat::Yaml)
        );
        assert_eq!(
            ExportFormat::from_extension("yml"),
            Some(ExportFormat::Yaml)
        );
        assert_eq!(ExportFormat::from_extension("unknown"), None);
    }

    #[test]
    fn test_export_format_mime_type() {
        assert_eq!(ExportFormat::Console.mime_type(), None);
        assert_eq!(ExportFormat::Json.mime_type(), Some("application/json"));
        assert_eq!(ExportFormat::Csv.mime_type(), Some("text/csv"));
        assert_eq!(
            ExportFormat::Tsv.mime_type(),
            Some("text/tab-separated-values")
        );
        assert_eq!(ExportFormat::Xml.mime_type(), Some("application/xml"));
        assert_eq!(ExportFormat::Html.mime_type(), Some("text/html"));
        assert_eq!(ExportFormat::Markdown.mime_type(), Some("text/markdown"));
        assert_eq!(ExportFormat::Yaml.mime_type(), Some("application/x-yaml"));
        assert_eq!(ExportFormat::Custom("test".to_string()).mime_type(), None);
    }

    #[test]
    fn test_export_format_equality() {
        assert_eq!(ExportFormat::Json, ExportFormat::Json);
        assert_ne!(ExportFormat::Json, ExportFormat::Csv);
        assert_eq!(
            ExportFormat::Custom("test".to_string()),
            ExportFormat::Custom("test".to_string())
        );
        assert_ne!(
            ExportFormat::Custom("test".to_string()),
            ExportFormat::Custom("other".to_string())
        );
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
        let original = ExportFormat::Custom("test".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // ExportHints tests
    #[test]
    fn test_export_hints_new() {
        let hints = ExportHints::new(ExportFormat::Json);

        assert_eq!(hints.format, ExportFormat::Json);
        assert_eq!(hints.max_rows, None);
        assert_eq!(hints.include_headers, true);
        assert_eq!(hints.pretty_print, true);
        assert!(hints.custom_options.is_empty());
    }

    #[test]
    fn test_export_hints_default() {
        let hints = ExportHints::default();

        assert_eq!(hints.format, ExportFormat::Console);
        assert_eq!(hints.max_rows, None);
        assert_eq!(hints.include_headers, true);
        assert_eq!(hints.pretty_print, true);
        assert!(hints.custom_options.is_empty());
    }

    #[test]
    fn test_export_hints_fluent_interface() {
        let hints = ExportHints::new(ExportFormat::Csv)
            .max_rows(100)
            .include_headers(false)
            .pretty_print(false)
            .custom_option("delimiter", ",")
            .custom_option("quote", "\"");

        assert_eq!(hints.format, ExportFormat::Csv);
        assert_eq!(hints.max_rows, Some(100));
        assert_eq!(hints.include_headers, false);
        assert_eq!(hints.pretty_print, false);
        assert_eq!(hints.get_custom_option("delimiter"), Some(&",".to_string()));
        assert_eq!(hints.get_custom_option("quote"), Some(&"\"".to_string()));
    }

    #[test]
    fn test_export_hints_get_custom_option() {
        let hints = ExportHints::new(ExportFormat::Json)
            .custom_option("indent", "2")
            .custom_option("compact", "false");

        assert_eq!(hints.get_custom_option("indent"), Some(&"2".to_string()));
        assert_eq!(
            hints.get_custom_option("compact"),
            Some(&"false".to_string())
        );
        assert_eq!(hints.get_custom_option("nonexistent"), None);
    }

    #[test]
    fn test_export_hints_equality() {
        let hints1 = ExportHints::new(ExportFormat::Json).max_rows(100);
        let hints2 = ExportHints::new(ExportFormat::Json).max_rows(100);
        let hints3 = ExportHints::new(ExportFormat::Csv).max_rows(100);

        assert_eq!(hints1, hints2);
        assert_ne!(hints1, hints3);
    }

    #[test]
    fn test_export_hints_clone() {
        let original = ExportHints::new(ExportFormat::Json)
            .max_rows(50)
            .custom_option("test", "value");

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // PluginDataExport tests
    #[test]
    fn test_plugin_data_export_new() {
        let schema = DataSchema::new("test", "1.0");
        let rows = vec![Row::new(vec![Value::Integer(1)])];
        let payload = DataPayload::tabular(schema, rows);

        let export = PluginDataExport::new("test_plugin", "scan_123", payload.clone());

        assert_eq!(export.plugin_id, "test_plugin");
        assert_eq!(export.scan_id, "scan_123");
        assert_eq!(export.payload, payload);
        assert_eq!(export.hints, ExportHints::default());
        assert!(export.metadata.is_empty());
    }

    #[test]
    fn test_plugin_data_export_with_hints() {
        let payload = DataPayload::raw("test data".to_string(), None);
        let hints = ExportHints::new(ExportFormat::Json).max_rows(100);

        let export =
            PluginDataExport::new("test_plugin", "scan_123", payload).with_hints(hints.clone());

        assert_eq!(export.hints, hints);
    }

    #[test]
    fn test_plugin_data_export_with_metadata() {
        let payload = DataPayload::raw("test data".to_string(), None);

        let export = PluginDataExport::new("test_plugin", "scan_123", payload)
            .with_metadata("version", "1.0")
            .with_metadata("author", "test");

        assert_eq!(export.metadata.get("version"), Some(&"1.0".to_string()));
        assert_eq!(export.metadata.get("author"), Some(&"test".to_string()));
    }

    #[test]
    fn test_plugin_data_export_export_type() {
        let payload = DataPayload::key_value(HashMap::new());
        let export = PluginDataExport::new("test_plugin", "scan_123", payload);

        assert_eq!(export.export_type(), DataExportType::KeyValue);
    }

    #[test]
    fn test_plugin_data_export_estimated_size() {
        let schema = DataSchema::new("test", "1.0");
        let rows = vec![Row::new(vec![Value::String("test".to_string())])];
        let payload = DataPayload::tabular(schema, rows);

        let export =
            PluginDataExport::new("test_plugin", "scan_123", payload).with_metadata("key", "value");

        let size = export.estimated_size();
        assert!(size > 0);
        assert!(size > export.plugin_id.len() + export.scan_id.len());
    }

    #[test]
    fn test_plugin_data_export_equality() {
        let payload = DataPayload::raw("test".to_string(), None);

        // Note: We can't easily test equality due to SystemTime::now() in constructor
        // This test verifies the structure exists and can be compared
        let export1 = PluginDataExport::new("plugin1", "scan1", payload.clone());
        let export2 = PluginDataExport::new("plugin1", "scan1", payload);

        // They won't be equal due to different timestamps, but structure should be valid
        assert_eq!(export1.plugin_id, export2.plugin_id);
        assert_eq!(export1.scan_id, export2.scan_id);
        assert_eq!(export1.payload, export2.payload);
    }

    #[test]
    fn test_plugin_data_export_clone() {
        let payload = DataPayload::raw("test data".to_string(), None);
        let original =
            PluginDataExport::new("test_plugin", "scan_123", payload).with_metadata("key", "value");

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // PluginDataExportBuilder tests
    #[test]
    fn test_plugin_data_export_builder_basic() {
        let payload = DataPayload::raw("test data".to_string(), None);

        let export = PluginDataExport::builder("test_plugin", "scan_123")
            .payload(payload.clone())
            .build()
            .unwrap();

        assert_eq!(export.plugin_id, "test_plugin");
        assert_eq!(export.scan_id, "scan_123");
        assert_eq!(export.payload, payload);
    }

    #[test]
    fn test_plugin_data_export_builder_with_options() {
        let payload = DataPayload::raw("test data".to_string(), None);
        let hints = ExportHints::new(ExportFormat::Json).max_rows(50);
        let timestamp = SystemTime::now();

        let export = PluginDataExport::builder("test_plugin", "scan_123")
            .payload(payload.clone())
            .hints(hints.clone())
            .metadata("version", "1.0")
            .metadata("source", "test")
            .timestamp(timestamp)
            .build()
            .unwrap();

        assert_eq!(export.plugin_id, "test_plugin");
        assert_eq!(export.scan_id, "scan_123");
        assert_eq!(export.payload, payload);
        assert_eq!(export.hints, hints);
        assert_eq!(export.timestamp, timestamp);
        assert_eq!(export.metadata.get("version"), Some(&"1.0".to_string()));
        assert_eq!(export.metadata.get("source"), Some(&"test".to_string()));
    }

    #[test]
    fn test_plugin_data_export_builder_missing_payload() {
        let result = PluginDataExport::builder("test_plugin", "scan_123").build();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Payload is required");
    }

    #[test]
    fn test_plugin_data_export_builder_fluent_interface() {
        let payload = DataPayload::key_value(HashMap::new());
        let hints = ExportHints::new(ExportFormat::Csv);

        let export = PluginDataExport::builder("plugin", "scan")
            .payload(payload.clone())
            .hints(hints.clone())
            .metadata("test", "value")
            .build()
            .unwrap();

        assert_eq!(export.plugin_id, "plugin");
        assert_eq!(export.scan_id, "scan");
        assert_eq!(export.payload, payload);
        assert_eq!(export.hints, hints);
        assert_eq!(export.metadata.len(), 1);
    }
}
