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

    assert_eq!(functions.len(), 1);

    let export_func = &functions[0];
    assert_eq!(export_func.name, "export");
    assert_eq!(
        export_func.description,
        "Export processed data in various formats"
    );
    assert_eq!(export_func.aliases, vec!["output", "format"]);
}

#[tokio::test]
async fn test_requirements() {
    let plugin = OutputPlugin::new();

    // Default OutputPlugin should suppress progress (outputs to stdout by default)
    assert_eq!(plugin.requirements(), ScanRequires::SUPPRESS_PROGRESS);
}
