//! Tests for OutputPlugin progress suppression behavior
//!
//! These tests verify that the OutputPlugin properly returns ScanRequires::SUPPRESS_PROGRESS
//! when output is directed to stdout, and ScanRequires::NONE when output goes to files.

use crate::plugin::args::PluginConfig;
use crate::plugin::builtin::output::OutputPlugin;
use crate::plugin::traits::Plugin;
use crate::scanner::types::ScanRequires;

#[tokio::test]
async fn test_default_plugin_suppresses_progress() {
    // Default OutputPlugin should suppress progress (outputs to stdout by default)
    let plugin = OutputPlugin::new();

    let requirements = plugin.requirements();
    assert!(requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::SUPPRESS_PROGRESS);
}

#[tokio::test]
async fn test_stdout_destination_suppresses_progress() {
    let mut plugin = OutputPlugin::new();
    let config = PluginConfig::default();

    // Test explicit stdout argument
    let args = vec!["--output".to_string(), "stdout".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::SUPPRESS_PROGRESS);
}

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

#[tokio::test]
async fn test_empty_destination_suppresses_progress() {
    let mut plugin = OutputPlugin::new();
    let config = PluginConfig::default();

    // Test empty string (defaults to stdout)
    let args = vec!["--output".to_string(), "".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    let requirements = plugin.requirements();
    assert!(requirements.contains(ScanRequires::SUPPRESS_PROGRESS));
    assert_eq!(requirements, ScanRequires::SUPPRESS_PROGRESS);
}

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

    // Test --output=stdout syntax
    let args = vec!["--output=stdout".to_string()];
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
    config.set_string("output", "stdout");

    // Config-based output destination
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

    // Initially should suppress progress (default stdout)
    assert_eq!(plugin.requirements(), ScanRequires::SUPPRESS_PROGRESS);

    // After setting file output, should not suppress progress
    let config = PluginConfig::default();
    let args = vec!["--output".to_string(), "output.txt".to_string()];
    plugin.parse_plugin_arguments(&args, &config).await.unwrap();

    assert_eq!(plugin.requirements(), ScanRequires::NONE);
}
