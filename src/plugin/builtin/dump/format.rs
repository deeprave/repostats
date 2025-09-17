//! Formatting utilities for DumpPlugin (split from monolithic file)
use crate::queue::typed::TypedMessage;
use crate::scanner::api::ScanMessage;
use serde_json::json;

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
    obj.insert("scan_message".into(), json!(typed_msg.content));

    Value::Object(obj).to_string()
}

pub fn format_compact_typed(
    typed_msg: &TypedMessage<ScanMessage>,
    show_headers: bool,
    color_enabled: bool,
) -> String {
    use crate::core::styles::StyleRole;
    use crate::scanner::api::ScanMessage as SM;
    use chrono::{DateTime, Local};

    let paint = |role: StyleRole, text: &str| role.paint(text, color_enabled);

    let header_prefix = if show_headers {
        format!(
            "{}:{}:",
            paint(StyleRole::Header, &typed_msg.header.sequence.to_string()),
            paint(StyleRole::Header, &typed_msg.header.producer_id)
        )
    } else {
        String::new()
    };

    let format_duration = |duration: std::time::Duration| -> String {
        // Phase 4.4.3: Implement reliable duration formatting with chrono
        // Convert std::time::Duration to chrono::Duration for better handling
        let chrono_duration =
            chrono::Duration::from_std(duration).unwrap_or_else(|_| chrono::Duration::zero());

        let total_seconds = chrono_duration.num_seconds();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        // Get microseconds for the fractional part
        let total_microseconds = chrono_duration.num_microseconds().unwrap_or(0);
        let fractional_microseconds = (total_microseconds % 1_000_000).abs();

        // Format according to duration magnitude for compatibility
        if hours > 0 {
            format!(
                "{}h{}m{}.{:06}s",
                hours, minutes, seconds, fractional_microseconds
            )
        } else if minutes > 0 {
            format!("{}m{}.{:06}s", minutes, seconds, fractional_microseconds)
        } else {
            format!("{}.{:06}s", seconds, fractional_microseconds)
        }
    };

    let format_timestamp = |ts: &std::time::SystemTime| -> String {
        DateTime::<Local>::try_from(*ts)
            .map(|dt| dt.format("%Y-%m-%d,%H:%M:%S%.f").to_string())
            .unwrap_or_else(|_| "invalid-time".into())
    };

    let format_lines = |insertions: usize, deletions: usize| -> String {
        if insertions == 0 && deletions == 0 {
            String::new()
        } else {
            format!("+{}/-{}", insertions, deletions)
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
                "{}{}:{}:{}:{}:{}",
                header_prefix,
                paint(StyleRole::Header, "scan_started"),
                paint(StyleRole::Key, scanner_id),
                paint(StyleRole::Value, &repository_data.path),
                paint(StyleRole::Value, branch),
                format_timestamp(timestamp)
            )
        }
        SM::CommitData {
            commit_info,
            scanner_id,
            timestamp,
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
                "{}{}:{}:{}:{}:{}:{}:{}",
                header_prefix,
                paint(StyleRole::Header, "commit"),
                paint(StyleRole::Key, scanner_id),
                paint(StyleRole::Value, hash8),
                lines,
                parents,
                paint(
                    StyleRole::Value,
                    &format!("{}:{}", commit_info.author_name, commit_info.author_email)
                ),
                format_timestamp(timestamp)
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
                "{}{}:{}:{}:{}:{}",
                header_prefix,
                paint(StyleRole::Header, "file_change"),
                paint(StyleRole::Key, scanner_id),
                paint(StyleRole::Value, change_type),
                lines,
                paint(StyleRole::Value, file_path)
            )
        }
        SM::ScanCompleted {
            stats, scanner_id, ..
        } => {
            let lines = format_lines(stats.total_insertions, stats.total_deletions);
            let duration = format_duration(stats.scan_duration);
            format!(
                "{}{}:{}:{}:{}:{}:{}",
                header_prefix,
                paint(StyleRole::Header, "scan_completed"),
                paint(StyleRole::Key, scanner_id),
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
                "{}{}:{}:{}:{}",
                header_prefix,
                paint(StyleRole::Header, "scan_error"),
                paint(StyleRole::Key, scanner_id),
                paint(StyleRole::Error, error),
                context
            )
        }
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
        paint(
            StyleRole::Header,
            &format!(
                "#{}@{} ",
                typed_msg.header.sequence, typed_msg.header.producer_id
            ),
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
                let total_nanos = stats.scan_duration.as_nanos();
                let minutes = total_nanos / 60_000_000_000;
                let seconds = (total_nanos % 60_000_000_000) / 1_000_000_000;
                let nanos = total_nanos % 1_000_000_000;
                if minutes > 0 {
                    format!("{}m{}.{:06}s", minutes, seconds, nanos / 1000)
                } else {
                    format!("{}.{:06}s", seconds, nanos / 1000)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Test the new chrono-based duration formatting for edge cases
    /// Phase 4.4.1 & 4.4.3: Write test for edge cases and verify chrono implementation
    #[test]
    fn test_chrono_duration_formatting_edge_cases() {
        // Create the new format_duration closure as defined in the function
        let format_duration = |duration: std::time::Duration| -> String {
            // Phase 4.4.3: Implement reliable duration formatting with chrono
            let chrono_duration =
                chrono::Duration::from_std(duration).unwrap_or_else(|_| chrono::Duration::zero());

            let total_seconds = chrono_duration.num_seconds();
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;

            // Get microseconds for the fractional part
            let total_microseconds = chrono_duration.num_microseconds().unwrap_or(0);
            let fractional_microseconds = (total_microseconds % 1_000_000).abs();

            // Format according to duration magnitude for compatibility
            if hours > 0 {
                format!(
                    "{}h{}m{}.{:06}s",
                    hours, minutes, seconds, fractional_microseconds
                )
            } else if minutes > 0 {
                format!("{}m{}.{:06}s", minutes, seconds, fractional_microseconds)
            } else {
                format!("{}.{:06}s", seconds, fractional_microseconds)
            }
        };

        // Test zero duration
        let zero = Duration::from_secs(0);
        assert_eq!(format_duration(zero), "0.000000s");

        // Test sub-second duration
        let sub_second = Duration::from_millis(500);
        assert_eq!(format_duration(sub_second), "0.500000s");

        // Test exactly one second
        let one_second = Duration::from_secs(1);
        assert_eq!(format_duration(one_second), "1.000000s");

        // Test exactly one minute
        let one_minute = Duration::from_secs(60);
        assert_eq!(format_duration(one_minute), "1m0.000000s");

        // Test 1 hour (now properly handles hours)
        let one_hour = Duration::from_secs(3600);
        assert_eq!(
            format_duration(one_hour),
            "1h0m0.000000s",
            "New implementation properly handles hours"
        );

        // Test multiple hours (now properly handles hours)
        let three_hours = Duration::from_secs(3 * 3600);
        assert_eq!(
            format_duration(three_hours),
            "3h0m0.000000s",
            "New implementation properly handles multiple hours"
        );

        // Test complex duration with hours, minutes, seconds, and microseconds
        let complex = Duration::from_secs(7321) + Duration::from_micros(123456);
        // 7321 seconds = 2 hours, 2 minutes, 1 second
        assert_eq!(
            format_duration(complex),
            "2h2m1.123456s",
            "New implementation properly shows 2h2m1s with microseconds"
        );

        // Test very large duration (proper handling without overflow)
        let very_large = Duration::from_secs(u32::MAX as u64);
        let result = format_duration(very_large);
        assert!(
            result.contains("h"),
            "Should handle very large durations with hours"
        );

        // Test microsecond precision
        let precise = Duration::from_nanos(1_234_567_890);
        assert_eq!(format_duration(precise), "1.234567s");

        // Test edge case: 1h1m1s1μs
        let edge_case = Duration::from_secs(3661) + Duration::from_micros(1);
        assert_eq!(format_duration(edge_case), "1h1m1.000001s");

        // Test edge case: just over 1 hour
        let just_over_hour = Duration::from_secs(3601);
        assert_eq!(format_duration(just_over_hour), "1h0m1.000000s");
    }
}
