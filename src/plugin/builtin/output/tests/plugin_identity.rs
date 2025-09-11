//! Tests for OutputPlugin identity and metadata

use crate::plugin::builtin::output::OutputPlugin;
use crate::plugin::traits::Plugin;
use crate::plugin::types::PluginType;
use crate::scanner::types::ScanRequires;

#[tokio::test]
async fn test_plugin_type_is_output() {
    let plugin = OutputPlugin::new();

    assert_eq!(plugin.plugin_type(), PluginType::Output);
}

#[tokio::test]
async fn test_plugin_info_correctness() {
    let plugin = OutputPlugin::new();
    let info = plugin.plugin_info();

    assert_eq!(info.name, "output");
    assert_eq!(info.version, "1.0.0");
    assert_eq!(
        info.description,
        "Built-in output plugin for data export and formatting"
    );
    assert_eq!(info.author, "repostats built-in");
    assert_eq!(info.plugin_type, PluginType::Output);
    assert_eq!(info.required, ScanRequires::NONE);
    assert!(info.auto_active);
}

#[tokio::test]
async fn test_advertised_functions() {
    let plugin = OutputPlugin::new();
    let functions = plugin.advertised_functions();

    // Should advertise 8 functions: output, text, json, csv, tsv, xml, html, md, yaml
    assert_eq!(functions.len(), 9);

    // Find the main output function
    let output_func = functions
        .iter()
        .find(|name| *name == "output")
        .expect("output function should be advertised");
    assert_eq!(output_func, "output");

    // Verify all format-specific functions are advertised
    let expected_functions = vec!["text", "json", "csv", "tsv", "xml", "html", "md", "yaml"];
    for func_name in expected_functions {
        let func = functions
            .iter()
            .find(|name| **name == func_name)
            .expect(&format!("{} function should be advertised", func_name));
        assert_eq!(func, &func_name);
    }
}

// Removed test_requirements - replaced by more specific progress suppression tests
