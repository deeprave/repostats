//! CLI display utilities for formatting output

use crate::core::styles::StyleRole;
use crate::plugin::types::PluginInfo;
use prettytable::{Cell, Row, Table};
use std::io::Write;
use unicode_width::UnicodeWidthStr;

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
        if p.description.chars().any(|c| c.is_control()) {
            return Err(format!(
                "Invalid plugin '{}': control chars in description",
                p.name
            ));
        }
        for f in &p.functions {
            if f.chars().any(|c| c.is_control() || c == '\t') {
                return Err(format!("Invalid function '{}' in plugin '{}'", f, p.name));
            }
        }
    }

    // Create prettytable with custom minimalist format
    let mut table = Table::new();

    // Create custom format: no external borders, no lines between data rows, simple header separator
    use prettytable::format::{FormatBuilder, LinePosition, LineSeparator};
    let format = FormatBuilder::new()
        .column_separator(' ') // Single space between columns
        .borders(' ') // No external borders (use space)
        .separators(
            &[LinePosition::Title],
            LineSeparator::new('-', '-', '-', '-'),
        ) // Consistent dashes for proper alignment
        .padding(1, 1) // 1 space left and right padding
        .build();
    table.set_format(format);

    // Add header row using StyleRole for consistent colors
    let header_row = if use_color {
        let header_spec = StyleRole::Header.to_prettytable_spec().unwrap_or_default();
        Row::new(vec![
            Cell::new("Plugin").style_spec(&header_spec),
            Cell::new("Functions & Description").style_spec(&header_spec),
        ])
    } else {
        Row::new(vec![
            Cell::new("Plugin"),
            Cell::new("Functions & Description"),
        ])
    };
    table.set_titles(header_row);

    // Calculate max plugin name width (with a reasonable cap)
    let min_plugin_col_width = 8;
    let max_plugin_col_width = 32;
    let plugin_col_width = plugins
        .iter()
        .map(|p| UnicodeWidthStr::width(p.name.as_str()))
        .max()
        .map(|w| w.clamp(min_plugin_col_width, max_plugin_col_width))
        .unwrap_or(min_plugin_col_width);

    // Add data rows using prettytable's native colors
    for p in plugins {
        // Truncate or pad plugin name to fit column width
        let plugin_name = {
            let width = UnicodeWidthStr::width(p.name.as_str());
            if width > plugin_col_width {
                // Truncate and add ellipsis
                let mut s = String::new();
                let mut curr_width = 0;
                for c in p.name.chars() {
                    let cw = UnicodeWidthStr::width(c.to_string().as_str());
                    if curr_width + cw > plugin_col_width - 1 {
                        break;
                    }
                    s.push(c);
                    curr_width += cw;
                }
                s.push('…');
                s
            } else {
                // Return original name since it fits
                p.name.clone()
            }
        };

        let fn_list_plain = if p.functions.is_empty() {
            "(none)".to_string()
        } else {
            p.functions
                .iter()
                .map(|f| f.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };

        let plugin_name_cell = if use_color {
            let literal_spec = StyleRole::Literal.to_prettytable_spec().unwrap_or_default();
            Cell::new(&plugin_name).style_spec(&literal_spec)
        } else {
            Cell::new(&plugin_name)
        };

        // Function names should be colored but descriptions should be plain
        let functions_cell = if use_color {
            let fn_list_colored = if p.functions.is_empty() {
                "(none)".to_string()
            } else {
                p.functions
                    .iter()
                    .map(|f| StyleRole::Valid.paint(f, true))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let content = format!("{}\n{}", fn_list_colored, p.description); // Description uncolored
            Cell::new(&content)
        } else {
            let content = format!("{}\n{}", fn_list_plain, p.description);
            Cell::new(&content)
        };

        table.add_row(Row::new(vec![plugin_name_cell, functions_cell]));
    }

    // For now, just use regular print to the writer
    // Colors will work if the table has been created with proper styling
    table.print(&mut writer).map_err(|e| e.to_string())?;
    Ok(())
}

/// Convenience function for printing to stdout with proper color handling
pub fn display_plugin_table(plugins: Vec<PluginInfo>, use_color: bool) -> Result<(), String> {
    if plugins.is_empty() {
        println!("No plugins discovered.");
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
        if p.description.chars().any(|c| c.is_control()) {
            return Err(format!(
                "Invalid plugin '{}': control chars in description",
                p.name
            ));
        }
        for f in &p.functions {
            if f.chars().any(|c| c.is_control() || c == '\t') {
                return Err(format!("Invalid function '{}' in plugin '{}'", f, p.name));
            }
        }
    }

    // Create prettytable with custom minimalist format
    let mut table = Table::new();

    // Create custom format: no external borders, no lines between data rows, simple header separator
    use prettytable::format::{FormatBuilder, LinePosition, LineSeparator};
    let format = FormatBuilder::new()
        .column_separator(' ') // Single space between columns
        .borders(' ') // No external borders (use space)
        .separators(
            &[LinePosition::Title],
            LineSeparator::new('-', '-', '-', '-'),
        ) // Consistent dashes for proper alignment
        .padding(1, 1) // 1 space left and right padding
        .build();
    table.set_format(format);

    // Add header row using StyleRole for consistent colors
    let header_row = if use_color {
        let header_spec = StyleRole::Header.to_prettytable_spec().unwrap_or_default();
        Row::new(vec![
            Cell::new("Plugin").style_spec(&header_spec),
            Cell::new("Functions & Description").style_spec(&header_spec),
        ])
    } else {
        Row::new(vec![
            Cell::new("Plugin"),
            Cell::new("Functions & Description"),
        ])
    };
    table.set_titles(header_row);

    // Calculate max plugin name width (with a reasonable cap)
    let min_plugin_col_width = 8;
    let max_plugin_col_width = 32;
    let plugin_col_width = plugins
        .iter()
        .map(|p| UnicodeWidthStr::width(p.name.as_str()))
        .max()
        .map(|w| w.clamp(min_plugin_col_width, max_plugin_col_width))
        .unwrap_or(min_plugin_col_width);

    // Add data rows using prettytable's native colors
    for p in plugins {
        // Truncate or pad plugin name to fit column width
        let plugin_name = {
            let width = UnicodeWidthStr::width(p.name.as_str());
            if width > plugin_col_width {
                // Truncate and add ellipsis
                let mut s = String::new();
                let mut curr_width = 0;
                for c in p.name.chars() {
                    let cw = UnicodeWidthStr::width(c.to_string().as_str());
                    if curr_width + cw > plugin_col_width - 1 {
                        break;
                    }
                    s.push(c);
                    curr_width += cw;
                }
                s.push('…');
                s
            } else {
                // Return original name since it fits
                p.name.clone()
            }
        };

        let fn_list_plain = if p.functions.is_empty() {
            "(none)".to_string()
        } else {
            p.functions
                .iter()
                .map(|f| f.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };

        let plugin_name_cell = if use_color {
            let literal_spec = StyleRole::Literal.to_prettytable_spec().unwrap_or_default();
            Cell::new(&plugin_name).style_spec(&literal_spec)
        } else {
            Cell::new(&plugin_name)
        };

        // Function names should be colored but descriptions should be plain
        let functions_cell = if use_color {
            let fn_list_colored = if p.functions.is_empty() {
                "(none)".to_string()
            } else {
                p.functions
                    .iter()
                    .map(|f| StyleRole::Valid.paint(f, true))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let content = format!("{}\n{}", fn_list_colored, p.description); // Description uncolored
            Cell::new(&content)
        } else {
            let content = format!("{}\n{}", fn_list_plain, p.description);
            Cell::new(&content)
        };

        table.add_row(Row::new(vec![plugin_name_cell, functions_cell]));
    }

    // Use print_tty with force_colorize for stdout - this is the key fix!
    if use_color {
        table.print_tty(true).map_err(|e| e.to_string())?;
    } else {
        table.print_tty(false).map_err(|e| e.to_string())?;
    }

    Ok(())
}
