//! Markdown output formatter

use super::{FormatResult, OutputFormatter};
use crate::plugin::data_export::{DataPayload, ExportFormat, PluginDataExport, Value};

/// Markdown formatter implementation
pub struct MarkdownFormatter {
    include_toc: bool,
}

impl MarkdownFormatter {
    /// Create a new Markdown formatter
    pub fn new() -> Self {
        Self { include_toc: false }
    }

    /// Escape Markdown special characters
    fn escape_markdown(text: &str) -> String {
        text.chars()
            .map(|c| match c {
                '\\' => "\\\\".to_string(),
                '`' => "\\`".to_string(),
                '*' => "\\*".to_string(),
                '_' => "\\_".to_string(),
                '{' | '}' => format!("\\{}", c),
                '[' | ']' => format!("\\{}", c),
                '(' | ')' => format!("\\{}", c),
                '#' => "\\#".to_string(),
                '+' => "\\+".to_string(),
                '-' => "\\-".to_string(),
                '.' => "\\.".to_string(),
                '!' => "\\!".to_string(),
                '|' => "\\|".to_string(),
                _ => c.to_string(),
            })
            .collect()
    }

    /// Format a value for Markdown display
    fn format_value(&self, value: &Value) -> String {
        match value {
            Value::String(s) => Self::escape_markdown(s),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Boolean(b) => if *b { "✓" } else { "✗" }.to_string(),
            Value::Timestamp(ts) => format!("`{:?}`", ts),
            Value::Duration(d) => format!("`{:?}`", d),
            Value::Null => "*null*".to_string(),
        }
    }

    /// Format tabular data as Markdown table
    fn format_tabular(&self, rows: &[crate::plugin::data_export::Row]) -> String {
        if rows.is_empty() {
            return "*No data available*\n".to_string();
        }

        let mut markdown = String::new();

        // Determine max columns
        let max_cols = rows.iter().map(|r| r.values.len()).max().unwrap_or(0);

        // Add table header
        markdown.push_str("| ");
        for i in 0..max_cols {
            markdown.push_str(&format!("Column {} | ", i + 1));
        }
        markdown.push('\n');

        // Add separator row
        markdown.push_str("|");
        for _ in 0..max_cols {
            markdown.push_str(" --- |");
        }
        markdown.push('\n');

        // Add data rows
        for (row_idx, row) in rows.iter().enumerate() {
            markdown.push_str("| ");
            for i in 0..max_cols {
                if i < row.values.len() {
                    markdown.push_str(&self.format_value(&row.values[i]));
                }
                markdown.push_str(" | ");
            }
            markdown.push('\n');

            // Add metadata if present
            if !row.metadata.is_empty() {
                markdown.push_str(&format!("\n*Row {} metadata:* ", row_idx + 1));
                for (key, value) in &row.metadata {
                    markdown.push_str(&format!(
                        "**{}**: {}, ",
                        Self::escape_markdown(key),
                        Self::escape_markdown(value)
                    ));
                }
                markdown.push_str("\n\n");
            }
        }

        markdown
    }

    /// Format hierarchical data as Markdown
    fn format_hierarchical(
        &self,
        tree: &crate::plugin::data_export::TreeNode,
        depth: usize,
    ) -> String {
        let mut markdown = String::new();
        let indent = "  ".repeat(depth);
        let heading_level = std::cmp::min(depth + 1, 6); // Limit to h6
        let heading = "#".repeat(heading_level);

        // Add node as heading
        markdown.push_str(&format!(
            "{} {}\n\n",
            heading,
            Self::escape_markdown(&tree.key)
        ));

        // Add value
        markdown.push_str(&format!(
            "**Value:** {}\n\n",
            self.format_value(&tree.value)
        ));

        // Add metadata if present
        if !tree.metadata.is_empty() {
            markdown.push_str("**Metadata:**\n\n");
            for (key, value) in &tree.metadata {
                markdown.push_str(&format!(
                    "- **{}**: {}\n",
                    Self::escape_markdown(key),
                    Self::escape_markdown(value)
                ));
            }
            markdown.push('\n');
        }

        // Add children
        for child in &tree.children {
            markdown.push_str(&self.format_hierarchical(child, depth + 1));
        }

        markdown
    }

    /// Format key-value data as Markdown
    fn format_key_value(&self, data: &std::collections::HashMap<String, Value>) -> String {
        let mut markdown = String::from("## Key-Value Data\n\n");

        let mut keys: Vec<_> = data.keys().collect();
        keys.sort();

        // Check if we can create a nice table
        let has_complex_values = keys.iter().any(|key| {
            matches!(data.get(*key), Some(Value::String(s)) if s.contains('\n') || s.len() > 50)
        });

        if has_complex_values {
            // Use definition list style for complex values
            for key in keys {
                if let Some(value) = data.get(key) {
                    markdown.push_str(&format!(
                        "**{}**\n: {}\n\n",
                        Self::escape_markdown(key),
                        self.format_value(value)
                    ));
                }
            }
        } else {
            // Use table for simple values
            markdown.push_str("| Key | Value |\n");
            markdown.push_str("| --- | --- |\n");

            for key in keys {
                if let Some(value) = data.get(key) {
                    markdown.push_str(&format!(
                        "| {} | {} |\n",
                        Self::escape_markdown(key),
                        self.format_value(value)
                    ));
                }
            }
        }

        markdown
    }

    /// Generate table of contents
    fn generate_toc(&self, content: &str) -> String {
        let mut toc = String::from("## Table of Contents\n\n");

        for line in content.lines() {
            if line.starts_with('#') {
                let level = line.chars().take_while(|&c| c == '#').count();
                if level <= 3 {
                    // Only include h1-h3 in TOC
                    let title = line.trim_start_matches('#').trim();
                    let indent = "  ".repeat(level.saturating_sub(1));
                    let link = title
                        .to_lowercase()
                        .replace(' ', "-")
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == '-')
                        .collect::<String>();

                    toc.push_str(&format!("{}* [{}](#{})\n", indent, title, link));
                }
            }
        }

        toc.push('\n');
        toc
    }

    /// Wrap content with document structure
    fn wrap_document(&self, content: &str, title: &str) -> String {
        let mut markdown = String::new();

        // Add title
        markdown.push_str(&format!("# {}\n\n", Self::escape_markdown(title)));

        // Add TOC if requested
        if self.include_toc && content.contains('#') {
            markdown.push_str(&self.generate_toc(content));
        }

        // Add main content
        markdown.push_str(content);

        // Add footer
        markdown.push_str("\n---\n\n");
        markdown.push_str("*Generated by RepoStats*\n");

        markdown
    }
}

impl OutputFormatter for MarkdownFormatter {
    fn format(&self, data: &PluginDataExport, _use_colors: bool) -> FormatResult {
        // Markdown doesn't use terminal colors
        let content = match &data.payload {
            DataPayload::Tabular { rows, .. } => {
                let mut md = String::from("## Tabular Data\n\n");
                md.push_str(&self.format_tabular(rows));
                md
            }
            DataPayload::Hierarchical { roots } => {
                let mut md = String::from("## Hierarchical Data\n\n");
                for root in roots.iter() {
                    md.push_str(&self.format_hierarchical(root, 1));
                }
                md
            }
            DataPayload::KeyValue { data } => self.format_key_value(data),
            DataPayload::Raw { data, content_type } => {
                let mut md = String::from("## Raw Data\n\n");
                if let Some(ct) = content_type {
                    md.push_str(&format!(
                        "**Content-Type:** `{}`\n\n",
                        Self::escape_markdown(ct)
                    ));
                }

                // Determine if content should be in code block
                if data.contains('\n') || data.len() > 100 {
                    md.push_str("```\n");
                    md.push_str(data); // Don't escape inside code blocks
                    md.push_str("\n```\n");
                } else {
                    md.push_str(&format!("`{}`\n", data.replace('`', "\\`")));
                }
                md
            }
        };

        let title = &data.plugin_id;
        Ok(self.wrap_document(&content, title))
    }

    fn format_type(&self) -> ExportFormat {
        ExportFormat::Markdown
    }
}
