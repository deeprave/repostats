//! Formatting utilities for DumpPlugin (split from monolithic file)
use crate::plugin::args::OutputFormat;
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

    let header_prefix = if show_headers {
        format!(
            "{}:{}:",
            typed_msg.header.sequence, typed_msg.header.producer_id
        )
    } else {
        String::new()
    };

    match &typed_msg.content {
        // (trimmed vs original for brevity)
        SM::ScanStarted {
            repository_data,
            scanner_id,
            ..
        } => {
            let branch = repository_data
                .git_ref
                .as_deref()
                .or(repository_data.default_branch.as_deref())
                .unwrap_or("(default)");
            format!("{}scan_started:id={}::path={}::branch={}::files={}::authors={}::max_commits={}::date={}",
                header_prefix,
                scanner_id,
                repository_data.path,
                branch,
                repository_data.file_paths.as_deref().unwrap_or("all"),
                repository_data.authors.as_deref().unwrap_or("all"),
                repository_data.max_commits.map(|v| v.to_string()).unwrap_or_else(|| "none".into()),
                repository_data.date_range.as_deref().unwrap_or("all")
            )
        }
        SM::CommitData { commit_info, .. } => {
            let hash8 = if commit_info.hash.len() > 8 {
                &commit_info.hash[..8]
            } else {
                &commit_info.hash
            };
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
                "{}commit:{}:lines=+{}/-{}:parents={}:author=\"{} <{}>\"",
                header_prefix,
                hash8,
                commit_info.insertions,
                commit_info.deletions,
                parents,
                commit_info.author_name,
                commit_info.author_email
            )
        }
        _ => serde_json::to_string(&typed_msg.content).unwrap_or_default(),
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
    if show_headers {
        format!(
            "[{}] {} from {}: {}",
            typed_msg.header.sequence,
            typed_msg.header.message_type,
            typed_msg.header.producer_id,
            serde_json::to_string(&typed_msg.content).unwrap_or_default()
        )
    } else {
        serde_json::to_string(&typed_msg.content).unwrap_or_default()
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
        _ => format!(
            "{header_prefix}{}",
            serde_json::to_string(&typed_msg.content).unwrap_or_default()
        ),
    }
}
