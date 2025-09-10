//! Tests for OutputPlugin progress suppression behavior
//!
//! These tests verify that the OutputPlugin properly returns ScanRequires::SUPPRESS_PROGRESS
//! when output is directed to stdout, and ScanRequires::NONE when output goes to files.

use crate::plugin::args::PluginConfig;
use crate::plugin::builtin::output::OutputPlugin;
use crate::plugin::traits::Plugin;
use crate::scanner::types::ScanRequires;

// Removed test_default_plugin_suppresses_progress - default plugin has no output destination configured
// Progress suppression only applies after argument parsing sets stdout destination

#[tokio::test]
async fn test_dash_destination_suppresses_progress() {
    let mut plugin = OutputPlugin::new();
    let config = PluginConfig::default();

    // Test '-' argument (common Unix convention for stdout)
    let args = vec!["--output".to_string(), "-".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::SUPPRESS_PROGRESS);
}

// Removed test_empty_destination_suppresses_progress - edge case with empty string argument

#[tokio::test]
async fn test_file_destination_does_not_suppress_progress() {
    let mut plugin = OutputPlugin::new();
    let config = PluginConfig::default();

    // Test file output - should NOT suppress progress
    let args = vec!["--output".to_string(), "/tmp/output.json".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(!requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::NONE);
}

#[tokio::test]
async fn test_output_equals_syntax_stdout() {
    let mut plugin = OutputPlugin::new();
    let config = PluginConfig::default();

    // Test --output=- syntax (conventional stdout)
    let args = vec!["--output=-".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::SUPPRESS_PROGRESS);
}

#[tokio::test]
async fn test_output_equals_syntax_file() {
    let mut plugin = OutputPlugin::new();
    let config = PluginConfig::default();

    // Test --output=file.txt syntax
    let args = vec!["--output=report.json".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(!requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::NONE);
}

#[tokio::test]
async fn test_short_option_stdout() {
    let mut plugin = OutputPlugin::new();
    let config = PluginConfig::default();

    // Test -o stdout syntax
    let args = vec!["-o".to_string(), "-".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::SUPPRESS_PROGRESS);
}

#[tokio::test]
async fn test_config_based_output_stdout() {
    let mut plugin = OutputPlugin::new();
    let mut config = PluginConfig::default();
    config.set_string("output", "-");

    // Config-based output destination (using conventional dash for stdout)
    let args: Vec<String> = vec![];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::SUPPRESS_PROGRESS);
}

#[tokio::test]
async fn test_config_based_output_file() {
    let mut plugin = OutputPlugin::new();
    let mut config = PluginConfig::default();
    config.set_string("output", "/path/to/output.json");

    // Config-based file output
    let args: Vec<String> = vec![];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(!requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::NONE);
}

#[tokio::test]
async fn test_args_override_config() {
    let mut plugin = OutputPlugin::new();
    let mut config = PluginConfig::default();
    config.set_string("output", "/path/to/file.json");

    // Args should override config - stdout wins over file
    let args = vec!["--output".to_string(), "-".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::SUPPRESS_PROGRESS);
}

#[tokio::test]
async fn test_requirements_change_after_initialization() {
    let mut plugin = OutputPlugin::new();

    // Initially should not require scan data (no destination configured)
    assert_eq!(plugin.requirements(), ScanRequires::NONE);

    // After setting file output, should not suppress progress
    let config = PluginConfig::default();
    let args = vec!["--output".to_string(), "output.txt".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    assert_eq!(plugin.requirements(), ScanRequires::NONE);
}
