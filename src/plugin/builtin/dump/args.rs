//! Argument parsing for DumpPlugin
use crate::plugin::args::{PluginArgParser, PluginConfig};
use crate::plugin::builtin::dump::DumpPlugin;
use crate::plugin::builtin::dump::OutputFormat;
use crate::plugin::error::PluginResult;
use crate::plugin::traits::Plugin; // for plugin_info()
use clap::{Arg, ArgMatches};
use std::path::PathBuf;

impl DumpPlugin {
    pub(super) async fn args_parse(
        &mut self,
        args: &[String],
        config: &PluginConfig,
    ) -> PluginResult<()> {
        let info = self.plugin_info();
        let parser = PluginArgParser::new(
            &info.name,
            &info.description,
            &info.version,
            config.use_colors,
        )
        .args(Self::format_args())
        .arg(
            Arg::new("no-headers")
                .short('n')
                .long("no-headers")
                .action(clap::ArgAction::SetTrue)
                .help("Don't show message headers"),
        )
        .arg(
            Arg::new("checkout")
                .short('c')
                .long("checkout")
                .action(clap::ArgAction::SetTrue)
                .help("Request file content (historical reconstruction)"),
        )
        .arg(
            Arg::new("files")
                .short('f')
                .long("files")
                .action(clap::ArgAction::SetTrue)
                .help("Include file change metadata"),
        )
        .arg(
            Arg::new("outfile")
                .short('o')
                .long("outfile")
                .value_name("FILE")
                .help("Write output to FILE (overwrite). Colours disabled for files.")
                .value_parser(clap::value_parser!(PathBuf)),
        );

        let matches = parser.parse(args)?;
        self.output_format = Self::determine_format(&matches, config);
        self.show_headers = !matches.get_flag("no-headers");
        self.request_file_content = matches.get_flag("checkout");
        self.request_file_info = matches.get_flag("files");
        self.output_file = matches.get_one::<PathBuf>("outfile").cloned();
        self.use_colors = config.use_colors;

        Ok(())
    }

    /// Create standard format arguments for output plugins
    pub fn format_args() -> Vec<Arg> {
        vec![
            Arg::new("json")
                .short('J')
                .long("json")
                .action(clap::ArgAction::SetTrue)
                .help("Output in JSON format")
                .conflicts_with_all(&["text", "compact"]),
            Arg::new("text")
                .short('T')
                .long("text")
                .action(clap::ArgAction::SetTrue)
                .help("Output in human-readable text format (default)")
                .conflicts_with_all(&["json", "compact"]),
            Arg::new("compact")
                .short('C')
                .long("compact")
                .action(clap::ArgAction::SetTrue)
                .help("Output in compact single-line format")
                .conflicts_with_all(&["json", "text"]),
        ]
    }

    /// Determine output format from parsed arguments and config
    pub fn determine_format(matches: &ArgMatches, config: &PluginConfig) -> OutputFormat {
        if matches.get_flag("json") {
            return OutputFormat::Json;
        }
        if matches.get_flag("compact") {
            return OutputFormat::Compact;
        }
        if matches.get_flag("text") {
            return OutputFormat::Text;
        }

        // Check TOML configuration for default format
        match config
            .get_string("default_format", "text")
            .to_lowercase()
            .as_str()
        {
            "json" => OutputFormat::Json,
            "compact" => OutputFormat::Compact,
            _ => OutputFormat::Text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::builtin::dump::OutputFormat;

    #[test]
    fn test_plugin_config_default() {
        let config = PluginConfig::default();
        assert!(!config.use_colors);
        assert!(config.toml_config.is_empty());
    }

    #[test]
    fn test_plugin_config_get_methods() {
        let mut config = PluginConfig::default();
        config.toml_config.insert(
            "test_key".to_string(),
            toml::Value::String("test_value".to_string()),
        );
        config
            .toml_config
            .insert("test_bool".to_string(), toml::Value::Boolean(true));

        assert_eq!(config.get_string("test_key", "default"), "test_value");
        assert_eq!(config.get_string("missing_key", "default"), "default");
        assert!(config.get_bool("test_bool", false));
        assert!(!config.get_bool("missing_bool", false));
    }

    #[test]
    fn test_plugin_arg_parser() {
        let parser = PluginArgParser::new("test", "Test plugin", "1.0.0", false)
            .args(DumpPlugin::format_args());

        let matches = parser
            .parse(&["dump".to_string(), "--json".to_string()])
            .unwrap();
        assert!(matches.get_flag("json"));
    }

    #[test]
    fn test_determine_format() {
        let parser = PluginArgParser::new("test", "Test plugin", "1.0.0", false)
            .args(DumpPlugin::format_args());
        let config = PluginConfig::default();

        // Test JSON format
        let matches = parser
            .parse(&["dump".to_string(), "--json".to_string()])
            .unwrap();
        assert!(matches.get_flag("json"), "JSON flag should be true");
        assert!(!matches.get_flag("text"), "Text flag should be false");
        assert!(!matches.get_flag("compact"), "Compact flag should be false");
        assert_eq!(
            DumpPlugin::determine_format(&matches, &config),
            OutputFormat::Json
        );

        // Test compact format
        let matches = parser
            .parse(&["dump".to_string(), "--compact".to_string()])
            .unwrap();
        assert!(matches.get_flag("compact"), "Compact flag should be true");
        assert!(!matches.get_flag("json"), "JSON flag should be false");
        assert!(!matches.get_flag("text"), "Text flag should be false");
        assert_eq!(
            DumpPlugin::determine_format(&matches, &config),
            OutputFormat::Compact
        );

        // Test default (no flags)
        let matches = parser.parse(&["dump".to_string()]).unwrap();
        assert!(
            !matches.get_flag("json"),
            "JSON flag should be false by default"
        );
        assert!(
            !matches.get_flag("text"),
            "Text flag should be false by default"
        );
        assert!(
            !matches.get_flag("compact"),
            "Compact flag should be false by default"
        );
        assert_eq!(
            DumpPlugin::determine_format(&matches, &config),
            OutputFormat::Text
        );
    }
}
