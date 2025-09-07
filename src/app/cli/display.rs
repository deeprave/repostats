//! CLI display utilities for formatting output
//!
//! This module intentionally ignores the custom global style palette for the plugin
//! table to simplify colour handling while alignment issues are resolved.

use crate::core::styles::{style_role_to_tabled_color, StyleRole}; // unified style roles
use crate::plugin::types::PluginInfo;
use std::io::{self, Write};
use tabled::{
    settings::{
        object::{Cell, Columns, Rows},
        Alignment, Modify, Style, Width,
    },
    Table, Tabled,
};
use unicode_width::UnicodeWidthStr;

const PLUGIN_COLUMN_MIN_WIDTH: usize = 8;

#[derive(Tabled)]
struct DisplayRow {
    #[tabled(rename = "Plugin")]
    plugin: String,
    #[tabled(rename = "Functions & Description")]
    content: String, // functions list (possibly colored) then newline then description
}

/// Display plugin table to a provided writer for improved testability and composability
pub fn display_plugin_table_to_writer<W: Write>(
    plugins: Vec<PluginInfo>,
    use_color: bool,
    mut writer: W,
) -> Result<(), String> {
    if plugins.is_empty() {
        writeln!(writer, "No plugins discovered.").map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Basic validation to avoid control characters disrupting table layout
    for p in &plugins {
        if p.name.is_empty() {
            return Err("Invalid plugin: empty name".into());
        }
        if p.name.chars().any(|c| c.is_control() || c == '\t') {
            return Err(format!(
                "Invalid plugin '{}': control chars in name",
                p.name
            ));
        }
        if p.description.chars().any(|c| c.is_control() && c != ' ') {
            return Err(format!(
                "Invalid plugin '{}': control chars in description",
                p.name
            ));
        }
        for f in &p.functions {
            if f.name.chars().any(|c| c.is_control() || c == '\t') {
                return Err(format!(
                    "Invalid function '{}' in plugin '{}'",
                    f.name, p.name
                ));
            }
        }
    }

    // Build rows (single content column, multi-line: functions list then description)
    let plugin_rows: Vec<DisplayRow> = plugins
        .into_iter()
        .map(|p| {
            let fn_list_plain = if p.functions.is_empty() {
                "(none)".to_string()
            } else {
                p.functions
                    .iter()
                    .map(|f| f.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let fn_list = if use_color {
                StyleRole::Valid.paint(&fn_list_plain, true)
            } else {
                fn_list_plain
            };
            let content = format!("{}\n{}", fn_list, p.description);
            DisplayRow {
                plugin: p.name,
                content,
            }
        })
        .collect();

    // Determine plugin column width using Unicode width for proper alignment
    let plugin_width = plugin_rows
        .iter()
        .map(|r| r.plugin.width())
        .max()
        .map(|w| w.max(PLUGIN_COLUMN_MIN_WIDTH))
        .unwrap_or(PLUGIN_COLUMN_MIN_WIDTH);

    let mut table = Table::new(plugin_rows);
    table
        .with(Style::modern()) // Use tabled's built-in modern style for visual separation
        .with(Modify::new(Columns::new(0..1)).with(Width::wrap(plugin_width)))
        .with(Modify::new(Columns::new(0..2)).with(Alignment::left()));

    if use_color {
        // Header row styling
        if let Some(c) = style_role_to_tabled_color(StyleRole::Header) {
            table.with(Modify::new(Rows::new(0..1)).with(c));
        }
        // Plugin name column styling for data rows (starting from row 1, since row 0 is header)
        let total_rows = table.count_rows();
        for r in 1..total_rows {
            if let Some(c) = style_role_to_tabled_color(StyleRole::Literal) {
                table.with(Modify::new(Cell::new(r, 0)).with(c));
            }
        }
    }

    writeln!(writer, "{}", table.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

/// Convenience function for printing to stdout
pub fn display_plugin_table(plugins: Vec<PluginInfo>, use_color: bool) -> Result<(), String> {
    display_plugin_table_to_writer(plugins, use_color, io::stdout())
}
