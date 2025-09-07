//! Additional tests for DumpPlugin CLI mapping & FILE_INFO behavior
use crate::plugin::args::{OutputFormat, PluginConfig};
use crate::plugin::builtin::dump::DumpPlugin;
use crate::plugin::traits::Plugin; // bring trait for parse_plugin_arguments
use crate::queue::api::MessageHeader;
use crate::queue::typed::TypedMessage;
use crate::scanner::types::{RepositoryData, ScanMessage, ScanStats};
use std::time::{Duration, SystemTime};

fn ts(s: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(s)
}
fn hdr(seq: u64, ty: &str) -> MessageHeader {
    MessageHeader {
        sequence: seq,
        producer_id: "scn".into(),
        message_type: ty.to_string(),
        timestamp: ts(1_900_000_000),
    }
}

#[test]
fn cli_text_flag_maps_to_pretty() {
    let mut plugin = DumpPlugin::new();
    let cfg = PluginConfig {
        use_colors: Some(false),
        ..Default::default()
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        plugin
            .parse_plugin_arguments(&["--text".into()], &cfg)
            .await
            .unwrap();
    });
    assert!(matches!(plugin.test_output_format(), OutputFormat::Text));
}

#[test]
fn cli_default_output_format() {
    let mut plugin = DumpPlugin::new();
    let cfg = PluginConfig {
        use_colors: Some(false),
        ..Default::default()
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        plugin.parse_plugin_arguments(&[], &cfg).await.unwrap();
    });
    // When no CLI flag is provided, should default to Text format
    assert!(matches!(plugin.test_output_format(), OutputFormat::Text));
}

#[test]
fn json_output_scanner_id_no_prefix() {
    let mut repo_builder = RepositoryData::builder().with_repository("/repo");
    repo_builder.git_ref = Some("main".into());
    // Provide mandatory git_dir for builder
    repo_builder.git_dir = Some("/repo/.git".into());
    let repo = repo_builder.build().unwrap();
    let content = ScanMessage::ScanStarted {
        scanner_id: "0123456789abcdef".into(),
        timestamp: ts(1),
        repository_data: repo,
    };
    let msg = TypedMessage {
        header: hdr(1, content.message_type()),
        content,
    };
    let json_line = DumpPlugin::format_typed_message_direct(&msg, OutputFormat::Json, true, false);
    assert!(
        json_line.contains("0123456789abcdef"),
        "Expected raw scanner id w/out prefix: {json_line}"
    );
    assert!(
        !json_line.contains("scan-0123456789abcdef"),
        "Should not contain legacy scan- prefix: {json_line}"
    );
}

#[test]
fn scanner_id_with_scan_prefix_not_duplicated() {
    let mut repo_builder = RepositoryData::builder().with_repository("/repo");
    repo_builder.git_ref = Some("main".into());
    repo_builder.git_dir = Some("/repo/.git".into());
    let repo = repo_builder.build().unwrap();
    let content = ScanMessage::ScanStarted {
        scanner_id: "scan-0123456789abcdef".into(), // Input already has scan- prefix
        timestamp: ts(1),
        repository_data: repo,
    };
    let msg = TypedMessage {
        header: hdr(1, content.message_type()),
        content,
    };
    let json_line = DumpPlugin::format_typed_message_direct(&msg, OutputFormat::Json, true, false);
    assert!(
        json_line.contains("scan-0123456789abcdef"),
        "Expected scanner id with prefix preserved: {json_line}"
    );
    // Should not double the prefix
    assert!(
        !json_line.contains("scan-scan-0123456789abcdef"),
        "Should not duplicate scan- prefix: {json_line}"
    );
}
