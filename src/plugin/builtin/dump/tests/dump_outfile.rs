//! Tests for --outfile behaviour (colors disabled when directing to file)
use crate::plugin::args::{OutputFormat, PluginConfig};
use crate::plugin::builtin::dump::DumpPlugin;
use crate::plugin::traits::Plugin;
use tempfile::TempDir;

#[tokio::test]
async fn outfile_disables_color_and_writes_lines() {
    let mut plugin = DumpPlugin::new();
    let cfg = PluginConfig {
        use_colors: Some(true),
        ..Default::default()
    }; // forced colors but outfile disables
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("out.txt");

    // parse args with outfile
    plugin
        .parse_plugin_arguments(
            &[
                "--text".into(),
                "--outfile".into(),
                path.to_string_lossy().into(),
            ],
            &cfg,
        )
        .await
        .unwrap();

    // Verify configuration
    assert!(matches!(plugin.test_output_format(), OutputFormat::Text));
    assert!(plugin.test_output_file().is_some());
    assert!(
        !plugin.test_use_colors(),
        "colors should be disabled when using --outfile"
    );

    // Verify output file path is correctly set
    assert_eq!(plugin.test_output_file(), Some(path.as_path()));

    // Note: Actual file writing happens in the consumer loop which requires
    // a QueueConsumer with messages. Testing actual file content would require
    // a more complex integration test setup with mock queue infrastructure.
}

// No force-color test: option removed
