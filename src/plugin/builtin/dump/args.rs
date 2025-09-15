//! Argument parsing for DumpPlugin
use super::DumpPlugin;
use crate::plugin::args::{create_format_args, determine_format, PluginArgParser, PluginConfig};
use crate::plugin::error::PluginResult;
use crate::plugin::traits::Plugin; // for plugin_info()
use clap::Arg;
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
        .args(create_format_args())
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
        self.output_format = determine_format(&matches, config);
        self.show_headers = !matches.get_flag("no-headers");
        self.request_file_content = matches.get_flag("checkout");
        self.request_file_info = matches.get_flag("files");
        self.output_file = matches.get_one::<PathBuf>("outfile").cloned();
        let auto = std::io::IsTerminal::is_terminal(&std::io::stdout());
        let base = config.use_colors.unwrap_or(auto);
        self.use_colors = if self.output_file.is_some() {
            false
        } else {
            base
        };
        Ok(())
    }
}
