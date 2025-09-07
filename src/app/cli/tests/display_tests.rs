//! Tests for CLI display utilities
//!
//! This module contains all tests for the display module, including
//! plugin table formatting, validation, and output handling.

use crate::app::cli::display::*;
use crate::plugin::types::{PluginFunction, PluginInfo, PluginType};
use crate::scanner::types::ScanRequires;

fn create_test_plugin(name: &str, description: &str, functions: Vec<&str>) -> PluginInfo {
    PluginInfo {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        description: description.to_string(),
        author: "Test Author".to_string(),
        api_version: 20250101,
        plugin_type: PluginType::Processing,
        functions: functions
            .into_iter()
            .map(|f| PluginFunction {
                name: f.to_string(),
                description: format!("{} function", f),
                aliases: vec![],
            })
            .collect(),
        required: ScanRequires::NONE,
        auto_active: false,
    }
}

#[test]
fn test_display_empty_plugins_list() {
    let plugins = vec![];
    // Should not panic and handle gracefully
    assert!(display_plugin_table(plugins, false).is_ok());
}

#[test]
fn test_display_single_plugin() {
    let plugins = vec![create_test_plugin("test", "A test plugin", vec!["cmd"])];
    assert!(display_plugin_table(plugins, false).is_ok());
}

#[test]
fn test_display_plugin_with_no_functions() {
    let plugins = vec![create_test_plugin("empty", "No functions plugin", vec![])];
    assert!(display_plugin_table(plugins, false).is_ok());
}

#[test]
fn test_display_plugin_with_multiple_functions() {
    let plugins = vec![create_test_plugin(
        "multi",
        "Multiple functions plugin",
        vec!["func1", "func2", "func3"],
    )];
    assert!(display_plugin_table(plugins, false).is_ok());
}

#[test]
fn test_display_multiple_plugins() {
    let plugins = vec![
        create_test_plugin("plugin1", "First plugin", vec!["cmd1"]),
        create_test_plugin("plugin2", "Second plugin", vec!["cmd2", "cmd3"]),
    ];
    assert!(display_plugin_table(plugins, false).is_ok());
}

#[test]
fn test_display_with_color_enabled() {
    let plugins = vec![create_test_plugin(
        "colored",
        "Color test plugin",
        vec!["test"],
    )];
    assert!(display_plugin_table(plugins, true).is_ok());
}

#[test]
fn test_display_with_color_disabled() {
    let plugins = vec![create_test_plugin(
        "plain",
        "Plain text plugin",
        vec!["test"],
    )];
    assert!(display_plugin_table(plugins, false).is_ok());
}

#[test]
fn test_display_long_plugin_name() {
    let plugins = vec![create_test_plugin(
        "very_long_plugin_name_exceeding_normal_width",
        "Long name test",
        vec!["test"],
    )];
    assert!(display_plugin_table(plugins, false).is_ok());
}

#[test]
fn test_validation_empty_plugin_name() {
    let mut plugin = create_test_plugin("test", "Test plugin", vec!["cmd"]);
    plugin.name = String::new(); // Empty name
    let plugins = vec![plugin];

    let result = display_plugin_table(plugins, false);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Invalid plugin: empty name");
}
