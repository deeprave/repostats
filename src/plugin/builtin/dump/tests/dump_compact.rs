//! Tests for refined compact formatting
use crate::plugin::args::OutputFormat;
use crate::plugin::builtin::dump::DumpPlugin;
use crate::queue::api::MessageHeader;
use crate::queue::typed::TypedMessage;
use crate::scanner::types::{
    ChangeType, CommitInfo, FileChangeData, RepositoryData, ScanMessage, ScanStats,
};
use std::time::{Duration, SystemTime};

fn ts(s: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(s)
}
fn hdr(seq: u64, ty: &str) -> MessageHeader {
    MessageHeader {
        sequence: seq,
        producer_id: "scan".into(),
        message_type: ty.to_string(),
        timestamp: ts(1_800_000_000),
    }
}

#[test]
fn compact_scan_started() {
    let mut builder = RepositoryData::builder().with_repository("/r");
    builder.git_ref = Some("main".into());
    builder.git_dir = Some("/r/.git".into());
    let repo = builder.build().unwrap();
    let content = ScanMessage::ScanStarted {
        scanner_id: "scan".into(),
        timestamp: ts(1),
        repository_data: repo,
    };
    let msg = TypedMessage {
        header: hdr(1, content.message_type()),
        content,
    };
    // let out = DumpPlugin::format_typed_message_direct(&msg, OutputFormat::Compact, true);
    let out = DumpPlugin::format_typed_message_direct(&msg, OutputFormat::Compact, true, false);
    assert!(
        out.contains("1:scan:scan_started:id=scan::path=/r::branch=main::files=all::authors=all::max_commits=none::date=all"),
        "Unexpected: {out}"
    );
}

#[test]
fn compact_scan_error_fallback_to_json() {
    let content = ScanMessage::ScanError {
        scanner_id: "scan".into(),
        timestamp: ts(1),
        error: "Test error".into(),
        context: "Test context".into(),
    };
    let msg = TypedMessage {
        header: hdr(1, content.message_type()),
        content,
    };
    let out = DumpPlugin::format_typed_message_direct(&msg, OutputFormat::Compact, true, false);
    // ScanError should fall back to JSON serialization since it has no specific compact format
    assert!(
        out.contains("\"error\""),
        "Expected JSON fallback with error field: {out}"
    );
    assert!(out.contains("Test error"), "Expected error message: {out}");
    assert!(out.contains("Test context"), "Expected context: {out}");
}

#[test]
fn compact_commit() {
    let commit = CommitInfo {
        hash: "h".into(),
        short_hash: "h".into(),
        author_name: "A".into(),
        author_email: "a@x".into(),
        committer_name: "A".into(),
        committer_email: "a@x".into(),
        timestamp: ts(2),
        message: "msg".into(),
        parent_hashes: vec![],
        insertions: 2,
        deletions: 1,
    };
    let content = ScanMessage::CommitData {
        scanner_id: "scan".into(),
        timestamp: ts(3),
        commit_info: commit,
    };
    let msg = TypedMessage {
        header: hdr(2, content.message_type()),
        content,
    };
    // let out = DumpPlugin::format_typed_message_direct(&msg, OutputFormat::Compact, true);
    let out = DumpPlugin::format_typed_message_direct(&msg, OutputFormat::Compact, true, false);
    assert!(
        out.contains("2:scan:commit:h:lines=+2/-1:parents=root:author=\"A <a@x>\""),
        "Unexpected commit: {out}"
    );
}
