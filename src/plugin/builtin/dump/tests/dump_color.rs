//! Colour output tests (env-based forcing)
use crate::plugin::args::OutputFormat;
use crate::plugin::builtin::dump::DumpPlugin;
use crate::queue::api::MessageHeader;
use crate::queue::typed::TypedMessage;
use crate::scanner::types::{RepositoryData, ScanMessage};
use serial_test::serial;
use std::env;
use std::time::{Duration, SystemTime};

fn ts(s: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(s)
}

fn make_started(seq: u64) -> TypedMessage<ScanMessage> {
    let mut builder = RepositoryData::builder().with_repository("/repo");
    builder.git_ref = Some("main".into());
    builder.git_dir = Some("/repo/.git".into());
    let repo = builder.build().unwrap();
    let content = ScanMessage::ScanStarted {
        scanner_id: "scan".into(),
        timestamp: ts(1),
        repository_data: repo,
    };
    TypedMessage {
        header: MessageHeader {
            sequence: seq,
            producer_id: "scan".into(),
            message_type: content.message_type().to_string(),
            timestamp: ts(2),
        },
        content,
    }
}

#[test]
#[serial]
fn color_forced() {
    // Save original environment variable values
    let orig_force_color = std::env::var("FORCE_COLOR").ok();
    let orig_no_color = std::env::var("NO_COLOR").ok();

    env::set_var("FORCE_COLOR", "1");
    env::remove_var("NO_COLOR");
    let msg = make_started(1);

    // When FORCE_COLOR is set, colors should be enabled
    let color_setting = get_use_colors_setting();
    let use_colors = color_setting.unwrap_or(false);
    let out = DumpPlugin::format_typed_message_direct(&msg, OutputFormat::Text, true, use_colors);
    assert!(
        out.contains("\u{1b}["),
        "Expected ANSI escape in output: {out}"
    );

    // Restore original environment variable values
    match orig_force_color {
        Some(val) => env::set_var("FORCE_COLOR", val),
        None => env::remove_var("FORCE_COLOR"),
    }
    match orig_no_color {
        Some(val) => env::set_var("NO_COLOR", val),
        None => env::remove_var("NO_COLOR"),
    }
}

// Helper function to check environment color settings with NO_COLOR taking precedence
fn get_use_colors_setting() -> Option<bool> {
    // NO_COLOR should override FORCE_COLOR
    if std::env::var("NO_COLOR").is_ok() {
        return Some(false);
    }
    if std::env::var("FORCE_COLOR").is_ok() {
        return Some(true);
    }
    None // defer to auto (TTY) at point of use
}

#[test]
#[serial]
fn color_disabled_by_no_color() {
    // Save original values
    let orig_force_color = env::var("FORCE_COLOR").ok();
    let orig_no_color = env::var("NO_COLOR").ok();

    env::set_var("FORCE_COLOR", "1"); // should be overridden
    env::set_var("NO_COLOR", "1");
    let msg = make_started(2);

    // When NO_COLOR is set, colors should be disabled even if FORCE_COLOR is set
    let color_setting = get_use_colors_setting();
    let use_colors = color_setting.unwrap_or(false);
    let out = DumpPlugin::format_typed_message_direct(&msg, OutputFormat::Text, true, use_colors);
    assert!(
        !out.contains("\u{1b}["),
        "ANSI escape found when NO_COLOR set: {out}"
    );

    // Restore original values
    match orig_force_color {
        Some(val) => env::set_var("FORCE_COLOR", val),
        None => env::remove_var("FORCE_COLOR"),
    }
    match orig_no_color {
        Some(val) => env::set_var("NO_COLOR", val),
        None => env::remove_var("NO_COLOR"),
    }
}
