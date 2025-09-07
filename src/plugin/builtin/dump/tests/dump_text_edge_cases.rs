//! Edge case tests for DumpPlugin pretty/legacy formatting
use crate::plugin::args::{create_format_args, determine_format, PluginArgParser};
use crate::plugin::args::{OutputFormat, PluginConfig};

#[test]
fn raw_format_via_config() {
    // Simulate no CLI flags, config requesting raw
    let mut cfg = PluginConfig::default();
    cfg.toml_config.insert(
        "default_format".to_string(),
        toml::Value::String("raw".to_string()),
    );
    let parser = PluginArgParser::new("dump", "Dump", "1.0", None).args(create_format_args());
    let matches = parser.parse(&[]).unwrap();
    let format = determine_format(&matches, &cfg);
    assert_eq!(
        format,
        OutputFormat::Raw,
        "Config should map to Raw variant"
    );
}

#[test]
fn invalid_format_in_config_defaults_to_expected() {
    // Simulate config with an invalid format value
    let mut cfg = PluginConfig::default();
    cfg.toml_config.insert(
        "default_format".to_string(),
        toml::Value::String("not_a_real_format".to_string()),
    );
    let parser = PluginArgParser::new("dump", "Dump", "1.0", None).args(create_format_args());
    let matches = parser.parse(&[]).unwrap();
    let format = determine_format(&matches, &cfg);
    assert_eq!(
        format,
        OutputFormat::Text,
        "Invalid config value should fall back to Text format"
    );
}

#[test]
fn empty_format_in_config_defaults_to_expected() {
    // Simulate config with an empty string as format
    let mut cfg = PluginConfig::default();
    cfg.toml_config.insert(
        "default_format".to_string(),
        toml::Value::String("".to_string()),
    );
    let parser = PluginArgParser::new("dump", "Dump", "1.0", None).args(create_format_args());
    let matches = parser.parse(&[]).unwrap();
    let format = determine_format(&matches, &cfg);
    assert_eq!(
        format,
        OutputFormat::Text,
        "Empty config value should fall back to Text format"
    );
}

#[test]
fn non_string_format_in_config_defaults_to_expected() {
    // Simulate config with a non-string value
    let mut cfg = PluginConfig::default();
    cfg.toml_config
        .insert("default_format".to_string(), toml::Value::Integer(123));
    let parser = PluginArgParser::new("dump", "Dump", "1.0", None).args(create_format_args());
    let matches = parser.parse(&[]).unwrap();
    let format = determine_format(&matches, &cfg);
    assert_eq!(
        format,
        OutputFormat::Text,
        "Non-string config value should fall back to Text format"
    );
}

#[test]
fn cli_flag_overrides_config() {
    // Config requests text but CLI flag requests json
    let mut cfg = PluginConfig::default();
    cfg.toml_config.insert(
        "default_format".to_string(),
        toml::Value::String("text".to_string()),
    );
    let parser = PluginArgParser::new("dump", "Dump", "1.0", None).args(create_format_args());
    let matches = parser.parse(&["--json".to_string()]).unwrap();
    let format = determine_format(&matches, &cfg);
    assert_eq!(
        format,
        OutputFormat::Json,
        "CLI flag should override config setting"
    );
}
