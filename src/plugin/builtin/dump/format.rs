//! Formatting utilities for DumpPlugin (split from monolithic file)
use crate::plugin::builtin::dump::OutputFormat;
use crate::queue::typed::TypedMessage;
use crate::scanner::api::ScanMessage;
use serde_json::json;

// Public (within crate::plugin::builtin::dump) helpers
pub(super) fn format_typed_message_direct(
    typed_msg: &TypedMessage<ScanMessage>,
    output_format: OutputFormat,
    show_headers: bool,
    use_colors: bool,
) -> String {
    match output_format {
        OutputFormat::Json => format_json_typed(typed_msg, show_headers, use_colors),
        OutputFormat::Compact => format_compact_typed(typed_msg, show_headers, use_colors),
        OutputFormat::Raw => format_text_typed(typed_msg, show_headers, use_colors),
        OutputFormat::Text => format_pretty_text_typed(typed_msg, show_headers, use_colors),
    }
}

pub(super) fn format_typed_message_direct_with_color(
    typed_msg: &TypedMessage<ScanMessage>,
    output_format: OutputFormat,
    show_headers: bool,
    use_colors: bool,
) -> String {
    match output_format {
        OutputFormat::Json => format_json_typed(typed_msg, show_headers, use_colors),
        OutputFormat::Compact => format_compact_typed(typed_msg, show_headers, use_colors),
        OutputFormat::Raw => format_text_typed(typed_msg, show_headers, use_colors),
        OutputFormat::Text => format_pretty_text_typed(typed_msg, show_headers, use_colors),
    }
}

pub fn format_json_typed(
    typed_msg: &TypedMessage<ScanMessage>,
    show_headers: bool,
    _color_enabled: bool,
) -> String {
    use serde_json::{Map, Value};
    let mut obj = Map::new();

    if show_headers {
        obj.insert("sequence".into(), json!(typed_msg.header.sequence));
        obj.insert("producer_id".into(), json!(typed_msg.header.producer_id));
    }

    let ts = typed_msg.header.timestamp;
    let duration = ts.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs() as i64;
    let nanos = duration.subsec_nanos();

    // Compose ISO8601 string for better precision and readability
    use chrono::{DateTime, Utc};
    let datetime =
        DateTime::<Utc>::from_timestamp(secs, nanos).unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
    let iso8601 = datetime.to_rfc3339();

    obj.insert("message_type".into(), json!(typed_msg.header.message_type));
    obj.insert("timestamp".into(), json!(iso8601));
    obj.insert("timestamp_seconds".into(), json!(secs));
    obj.insert("timestamp_nanos".into(), json!(nanos));
    obj.insert("scan_message".into(), json!(typed_msg.content));

    Value::Object(obj).to_string()
}

pub fn format_compact_typed(
    typed_msg: &TypedMessage<ScanMessage>,
    show_headers: bool,
    _color_enabled: bool,
) -> String {
    use crate::scanner::api::ScanMessage as SM;
    use chrono::{DateTime, Local};

    let header_prefix = if show_headers {
        format!(
            "{}:{}:",
            typed_msg.header.sequence, typed_msg.header.producer_id
        )
    } else {
        String::new()
    };

    let format_duration = |duration: std::time::Duration| -> String {
        let total_millis = duration.as_millis();
        let minutes = total_millis / 60_000;
        let seconds = (total_millis % 60_000) / 1_000;
        let millis = total_millis % 1_000;
        if minutes > 0 {
            format!("{}m{}.{}s", minutes, seconds, millis / 100)
        } else {
            format!("{}.{}s", seconds, millis / 100)
        }
    };

    let format_timestamp = |ts: &std::time::SystemTime| -> String {
        DateTime::<Local>::try_from(*ts)
            .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_else(|_| "invalid-time".into())
    };

    let format_lines = |insertions: usize, deletions: usize| -> String {
        if insertions == 0 && deletions == 0 {
            String::new()
        } else {
            format!("+{}/{}", insertions, deletions)
        }
    };

    match &typed_msg.content {
        SM::ScanStarted {
            repository_data,
            scanner_id,
            timestamp,
        } => {
            let branch = repository_data
                .git_ref
                .as_deref()
                .or(repository_data.default_branch.as_deref())
                .unwrap_or("(default)");
            format!(
                "{}scan_started:{}:{}:{}:{}",
                header_prefix,
                scanner_id,
                repository_data.path,
                branch,
                format_timestamp(timestamp)
            )
        }
        SM::CommitData {
            commit_info,
            scanner_id,
            ..
        } => {
            let hash8 = if commit_info.hash.len() > 8 {
                &commit_info.hash[..8]
            } else {
                &commit_info.hash
            };
            let lines = format_lines(commit_info.insertions, commit_info.deletions);
            let parents = if commit_info.parent_hashes.is_empty() {
                "root".into()
            } else {
                commit_info
                    .parent_hashes
                    .iter()
                    .map(|h| if h.len() > 8 { &h[..8] } else { h })
                    .collect::<Vec<_>>()
                    .join(",")
            };
            format!(
                "{}commit:{}:{}:{}:{}:{} <{}>",
                header_prefix,
                scanner_id,
                hash8,
                lines,
                parents,
                commit_info.author_name,
                commit_info.author_email
            )
        }
        SM::FileChange {
            file_path,
            change_data,
            scanner_id,
            ..
        } => {
            let change_type = match change_data.change_type {
                crate::scanner::types::ChangeType::Added => "A",
                crate::scanner::types::ChangeType::Modified => "M",
                crate::scanner::types::ChangeType::Deleted => "D",
                crate::scanner::types::ChangeType::Renamed => "R",
                crate::scanner::types::ChangeType::Copied => "C",
            };
            let lines = format_lines(change_data.insertions, change_data.deletions);
            format!(
                "{}file_change:{}:{}:{}:{}",
                header_prefix, scanner_id, change_type, lines, file_path
            )
        }
        SM::ScanCompleted {
            stats, scanner_id, ..
        } => {
            let lines = format_lines(stats.total_insertions, stats.total_deletions);
            let duration = format_duration(stats.scan_duration);
            format!(
                "{}scan_completed:{}:{}:{}:{}:{}",
                header_prefix,
                scanner_id,
                stats.total_commits,
                stats.total_files_changed,
                lines,
                duration
            )
        }
        SM::ScanError {
            error,
            context,
            scanner_id,
            ..
        } => {
            format!(
                "{}scan_error:{}:{}:{}",
                header_prefix, scanner_id, error, context
            )
        }
    }
}

pub(super) fn format_text_typed(
    typed_msg: &TypedMessage<ScanMessage>,
    show_headers: bool,
    color_enabled: bool,
) -> String {
    if typed_msg.header.message_type.starts_with("scan_started") {
        format_repository_data_text_typed(typed_msg, show_headers, color_enabled)
    } else {
        format_regular_message_text_typed(typed_msg, show_headers, color_enabled)
    }
}

fn format_repository_data_text_typed(
    typed_msg: &TypedMessage<ScanMessage>,
    show_headers: bool,
    color_enabled: bool,
) -> String {
    if let ScanMessage::ScanStarted {
        repository_data, ..
    } = &typed_msg.content
    {
        if show_headers {
            format!(
                "[{}] Repository Metadata:\n  Path: {}",
                typed_msg.header.sequence, repository_data.path
            )
        } else {
            format!("Repository: {}", repository_data.path)
        }
    } else {
        format_regular_message_text_typed(typed_msg, show_headers, color_enabled)
    }
}

fn format_regular_message_text_typed(
    typed_msg: &TypedMessage<ScanMessage>,
    show_headers: bool,
    _color_enabled: bool,
) -> String {
    use crate::scanner::api::ScanMessage as SM;
    use chrono::{DateTime, Local};

    let format_duration = |duration: std::time::Duration| -> String {
        let total_millis = duration.as_millis();
        let minutes = total_millis / 60_000;
        let seconds = (total_millis % 60_000) / 1_000;
        let millis = total_millis % 1_000;
        if minutes > 0 {
            format!("{}m{}.{}s", minutes, seconds, millis / 100)
        } else {
            format!("{}.{}s", seconds, millis / 100)
        }
    };

    let format_timestamp = |ts: &std::time::SystemTime| -> String {
        DateTime::<Local>::try_from(*ts)
            .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_else(|_| "invalid-time".into())
    };

    let format_lines = |insertions: usize, deletions: usize| -> String {
        if insertions == 0 && deletions == 0 {
            "no changes".to_string()
        } else {
            format!("+{}/{} lines", insertions, deletions)
        }
    };

    let content_text = match &typed_msg.content {
        SM::ScanStarted {
            repository_data,
            scanner_id,
            timestamp,
        } => {
            let branch = repository_data
                .git_ref
                .as_deref()
                .or(repository_data.default_branch.as_deref())
                .unwrap_or("default");
            format!(
                "Scan {} started for repository {} (branch: {}) at {}",
                scanner_id,
                repository_data.path,
                branch,
                format_timestamp(timestamp)
            )
        }
        SM::CommitData { commit_info, .. } => {
            let hash8 = if commit_info.hash.len() > 8 {
                &commit_info.hash[..8]
            } else {
                &commit_info.hash
            };
            format!(
                "Commit {} by {} <{}>: {} ({})",
                hash8,
                commit_info.author_name,
                commit_info.author_email,
                commit_info.message.lines().next().unwrap_or(""),
                format_lines(commit_info.insertions, commit_info.deletions)
            )
        }
        SM::FileChange {
            file_path,
            change_data,
            ..
        } => {
            let change_desc = match change_data.change_type {
                crate::scanner::types::ChangeType::Added => "added",
                crate::scanner::types::ChangeType::Modified => "modified",
                crate::scanner::types::ChangeType::Deleted => "deleted",
                crate::scanner::types::ChangeType::Renamed => "renamed",
                crate::scanner::types::ChangeType::Copied => "copied",
            };
            format!(
                "File {} {}: {}",
                file_path,
                change_desc,
                format_lines(change_data.insertions, change_data.deletions)
            )
        }
        SM::ScanCompleted {
            stats, timestamp, ..
        } => {
            format!(
                "Scan completed: {} commits, {} files, {} ({}) at {}",
                stats.total_commits,
                stats.total_files_changed,
                format_lines(stats.total_insertions, stats.total_deletions),
                format_duration(stats.scan_duration),
                format_timestamp(timestamp)
            )
        }
        SM::ScanError {
            error,
            context,
            timestamp,
            ..
        } => {
            format!(
                "Scan error at {}: {} (context: {})",
                format_timestamp(timestamp),
                error,
                context
            )
        }
    };

    if show_headers {
        format!(
            "[{}] {} from {}: {}",
            typed_msg.header.sequence,
            typed_msg.header.message_type,
            typed_msg.header.producer_id,
            content_text
        )
    } else {
        content_text
    }
}

pub fn format_pretty_text_typed(
    typed_msg: &TypedMessage<ScanMessage>,
    show_headers: bool,
    color_enabled: bool,
) -> String {
    use crate::core::styles::StyleRole;
    use crate::scanner::api::ScanMessage::*;
    use chrono::{DateTime, Local};
    let ts = |ts: &std::time::SystemTime| {
        DateTime::<Local>::try_from(*ts)
            .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_else(|_| "invalid-time".into())
    };
    let paint = |role: StyleRole, text: &str| role.paint(text, color_enabled);
    let label = |t: &str| paint(StyleRole::Header, t);
    let key = |k: &str| paint(StyleRole::Key, k);
    let kv = |k: &str, v: String| format!("{}={}", key(k), v);
    let kvs = |k: &str, v: &str| format!("{}={}", key(k), v);
    let header_prefix = if show_headers {
        format!(
            "#{}@{} ",
            typed_msg.header.sequence, typed_msg.header.producer_id
        )
    } else {
        String::new()
    };
    match &typed_msg.content {
        ScanStarted {
            repository_data,
            timestamp,
            scanner_id,
        } => {
            let mut parts = Vec::new();
            parts.push(label("ScanStarted"));
            parts.push(kvs("id", scanner_id));
            parts.push(kvs("repo", &repository_data.path));
            parts.push(kv("ts", ts(timestamp)));
            format!("{header_prefix}{}", parts.join(" "))
        }
        ScanCompleted {
            stats,
            timestamp,
            scanner_id,
        } => {
            let mut parts = Vec::new();
            parts.push(label("ScanCompleted"));
            parts.push(kvs("id", scanner_id));
            parts.push(kv("commits", stats.total_commits.to_string()));
            parts.push(kv("files", stats.total_files_changed.to_string()));
            let lines = if stats.total_insertions == 0 && stats.total_deletions == 0 {
                "no changes".to_string()
            } else {
                format!(
                    "+{}/-{} lines",
                    stats.total_insertions, stats.total_deletions
                )
            };
            parts.push(kvs("lines", &lines));
            let duration_str = {
                let total_millis = stats.scan_duration.as_millis();
                let minutes = total_millis / 60_000;
                let seconds = (total_millis % 60_000) / 1_000;
                let millis = total_millis % 1_000;
                if minutes > 0 {
                    format!("{}m{}.{}s", minutes, seconds, millis / 100)
                } else {
                    format!("{}.{}s", seconds, millis / 100)
                }
            };
            parts.push(kvs("duration", &duration_str));
            parts.push(kv("ts", ts(timestamp)));
            format!("{header_prefix}{}", parts.join(" "))
        }
        CommitData {
            commit_info,
            timestamp,
            scanner_id,
        } => {
            let mut parts = Vec::new();
            parts.push(label("CommitData"));
            parts.push(kvs("id", scanner_id));
            let hash8 = if commit_info.hash.len() > 8 {
                &commit_info.hash[..8]
            } else {
                &commit_info.hash
            };
            parts.push(kvs("commit", hash8));
            let lines = if commit_info.insertions == 0 && commit_info.deletions == 0 {
                "no changes".to_string()
            } else {
                format!("+{}/-{}", commit_info.insertions, commit_info.deletions)
            };
            parts.push(kvs("lines", &lines));
            parts.push(kvs(
                "author",
                &format!("{} <{}>", commit_info.author_name, commit_info.author_email),
            ));
            parts.push(kv("ts", ts(timestamp)));
            format!("{header_prefix}{}", parts.join(" "))
        }
        FileChange {
            file_path,
            change_data,
            timestamp,
            scanner_id,
            ..
        } => {
            let mut parts = Vec::new();
            parts.push(label("FileChange"));
            parts.push(kvs("id", scanner_id));
            let change_type = match change_data.change_type {
                crate::scanner::types::ChangeType::Added => "added",
                crate::scanner::types::ChangeType::Modified => "modified",
                crate::scanner::types::ChangeType::Deleted => "deleted",
                crate::scanner::types::ChangeType::Renamed => "renamed",
                crate::scanner::types::ChangeType::Copied => "copied",
            };
            parts.push(kvs("change", change_type));
            parts.push(kvs("file", file_path));
            let lines = if change_data.insertions == 0 && change_data.deletions == 0 {
                "no changes".to_string()
            } else {
                format!("+{}/-{}", change_data.insertions, change_data.deletions)
            };
            parts.push(kvs("lines", &lines));
            parts.push(kv("ts", ts(timestamp)));
            format!("{header_prefix}{}", parts.join(" "))
        }
        ScanError {
            error,
            context,
            timestamp,
            scanner_id,
        } => {
            let mut parts = Vec::new();
            parts.push(label("ScanError"));
            parts.push(kvs("id", scanner_id));
            parts.push(kvs("error", error));
            parts.push(kvs("context", context));
            parts.push(kv("ts", ts(timestamp)));
            format!("{header_prefix}{}", parts.join(" "))
        }
    }
}
