//! CLI display utilities for formatting output

use crate::plugin::types::PluginInfo;
use tabled::{
    settings::{
        object::{Columns, Object, Rows},
        Alignment, Color, Modify, Style, Width,
    },
    Table, Tabled,
};

/// Table formatting constants
const PLUGIN_COLUMN_WIDTH: usize = 8;
const TABLE_SEPARATOR: &str = "--------";
const SEPARATOR_SUFFIX: &str = "--";

/// Table row data structure for tabled
#[derive(Tabled)]
struct DisplayRow {
    #[tabled(rename = "Plugin")]
    plugin: String,
    #[tabled(rename = "Functions / Description")]
    content: String,
}

/// Display plugin information in a simple formatted table using tabled
/// Returns an error if plugin data is invalid or malformed
pub fn display_plugin_table(plugins: Vec<PluginInfo>, use_color: bool) -> Result<(), String> {
    if plugins.is_empty() {
        eprintln!("No plugins discovered.");
        return Ok(());
    }

    // Enhanced validation for table safety
    for plugin in &plugins {
        if plugin.name.is_empty() {
            return Err("Invalid plugin: empty name".to_string());
        }
        if plugin.name.chars().any(|c| c.is_control() || c == '\t') {
            return Err(format!(
                "Invalid plugin '{}': name contains control characters",
                plugin.name
            ));
        }
        if plugin
            .description
            .chars()
            .any(|c| c.is_control() && c != ' ')
        {
            return Err(format!(
                "Invalid plugin '{}': description contains control characters",
                plugin.name
            ));
        }
        // Validate function names as well
        for func in &plugin.functions {
            if func.name.chars().any(|c| c.is_control() || c == '\t') {
                return Err(format!(
                    "Invalid function '{}' in plugin '{}': contains control characters",
                    func.name, plugin.name
                ));
            }
        }
    }

    // Build table data with proper structure
    let mut table_data = Vec::new();

    for plugin in plugins {
        let function_names: Vec<String> = plugin
            .functions
            .iter()
            .map(|func| func.name.clone())
            .collect();

        let functions_text = function_names.join(", ");

        // Add plugin row with functions
        table_data.push(DisplayRow {
            plugin: plugin.name,
            content: functions_text,
        });

        // Add description row with empty plugin column
        table_data.push(DisplayRow {
            plugin: String::new(),
            content: plugin.description,
        });
    }

    // Create and configure table
    let mut table = Table::new(table_data);
    table
        .with(Style::empty())
        .with(Modify::new(Columns::single(0)).with(Width::wrap(PLUGIN_COLUMN_WIDTH)))
        .with(Modify::new(Columns::single(0)).with(Alignment::left()));

    // Apply colors if requested
    if use_color {
        table
            .with(Modify::new(Rows::single(0)).with(Color::FG_CYAN))
            .with(Modify::new(Columns::single(0).not(Rows::single(0))).with(Color::FG_BLUE));
    }

    // Output with custom separator
    print_table_with_separator(&table);

    Ok(())
}

/// Helper function to print tabled output with custom separator line
fn print_table_with_separator(table: &Table) {
    let table_output = table.to_string();
    let mut lines = table_output.lines();

    // Print header
    if let Some(header) = lines.next() {
        println!("{}", header);

        // Print custom separator
        println!(
            "{:<width$} {}",
            TABLE_SEPARATOR,
            SEPARATOR_SUFFIX,
            width = PLUGIN_COLUMN_WIDTH
        );

        // Print remaining data lines
        for line in lines {
            println!("{}", line);
        }
    }
}
