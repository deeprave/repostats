//! Date parsing utilities for CLI arguments
//!
//! Provides flexible date parsing supporting ISO 8601 formats and relative dates.

use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use std::time::SystemTime;

/// Parse a date string into a SystemTime
///
/// Supports:
/// - ISO 8601 dates: "2024-01-15", "2024-01-15T10:30:00Z"
/// - Relative past dates: "yesterday", "1 week ago", "2 months ago"
/// - Relative future dates: "in 2 days", "3 hours from now", "tomorrow"
pub fn parse_date(date_str: &str) -> Result<SystemTime, String> {
    // Try ISO 8601 date first
    if let Ok(system_time) = parse_iso_date(date_str) {
        return Ok(system_time);
    }

    // Try relative date
    if let Ok(system_time) = parse_relative_date(date_str) {
        return Ok(system_time);
    }

    Err(format!(
        "Invalid date format: '{}'. Expected ISO 8601 (YYYY-MM-DD), past relative (e.g., 'yesterday', '1 week ago'), or future relative (e.g., 'in 2 days', '3 hours from now')",
        date_str
    ))
}

/// Parse an ISO 8601 date string
fn parse_iso_date(date_str: &str) -> Result<SystemTime, String> {
    // Try full datetime first
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Ok(dt.with_timezone(&Utc).into());
    }

    // Try date with time
    if let Ok(dt) = date_str.parse::<DateTime<Utc>>() {
        return Ok(dt.into());
    }

    // Try just date (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        let datetime = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| "Invalid time".to_string())?;
        let dt = Utc
            .from_local_datetime(&datetime)
            .single()
            .ok_or_else(|| "Ambiguous or invalid local time".to_string())?;
        return Ok(dt.into());
    }

    Err("Not an ISO date".to_string())
}

/// Parse a relative date string
fn parse_relative_date(date_str: &str) -> Result<SystemTime, String> {
    let lower = date_str.to_lowercase();
    let now = Local::now();

    // Handle special cases
    match lower.as_str() {
        "now" | "today" => return Ok(SystemTime::now()),
        "yesterday" => {
            let yesterday = now - Duration::days(1);
            return Ok(yesterday.with_timezone(&Utc).into());
        }
        "tomorrow" => {
            let tomorrow = now + Duration::days(1);
            return Ok(tomorrow.with_timezone(&Utc).into());
        }
        _ => {}
    }

    // Parse patterns like "N <unit> ago" or "in N <unit>" or "N <unit> from now"
    let parts: Vec<&str> = lower.split_whitespace().collect();

    // Handle "N unit ago" pattern
    if parts.len() >= 2 && parts.last() == Some(&"ago") {
        let count = parts[0]
            .parse::<i64>()
            .map_err(|_| format!("Invalid number in relative date: '{}'", parts[0]))?;

        let unit = if parts.len() == 3 {
            parts[1]
        } else if parts.len() == 2 {
            // Handle cases like "1week ago" without space
            let combined = parts[0];
            if let Some(idx) = combined.chars().position(|c| c.is_alphabetic()) {
                &combined[idx..]
            } else {
                return Err("Invalid relative date format".to_string());
            }
        } else {
            return Err("Invalid relative date format".to_string());
        };

        let duration = parse_time_unit(unit, count)?;
        let past_time = now - duration;
        return Ok(past_time.with_timezone(&Utc).into());
    }

    // Handle "in N unit" pattern
    if parts.len() == 3 && parts[0] == "in" {
        let count = parts[1]
            .parse::<i64>()
            .map_err(|_| format!("Invalid number in relative date: '{}'", parts[1]))?;

        let unit = parts[2];
        let duration = parse_time_unit(unit, count)?;
        let future_time = now + duration;
        return Ok(future_time.with_timezone(&Utc).into());
    }

    // Handle "N unit from now" pattern
    if parts.len() >= 4 && parts[parts.len() - 2..] == ["from", "now"] {
        let count = parts[0]
            .parse::<i64>()
            .map_err(|_| format!("Invalid number in relative date: '{}'", parts[0]))?;

        let unit = parts[1];
        let duration = parse_time_unit(unit, count)?;
        let future_time = now + duration;
        return Ok(future_time.with_timezone(&Utc).into());
    }

    Err("Not a recognized relative date format".to_string())
}

/// Parse a time unit string and count into a chrono Duration
fn parse_time_unit(unit: &str, count: i64) -> Result<Duration, String> {
    match unit {
        "second" | "seconds" | "sec" | "secs" | "s" => Ok(Duration::seconds(count)),
        "minute" | "minutes" | "min" | "mins" | "m" => Ok(Duration::minutes(count)),
        "hour" | "hours" | "hr" | "hrs" | "h" => Ok(Duration::hours(count)),
        "day" | "days" | "d" => Ok(Duration::days(count)),
        "week" | "weeks" | "w" => Ok(Duration::weeks(count)),
        "month" | "months" => Ok(Duration::days(count * 30)), // Approximate
        "year" | "years" | "y" => Ok(Duration::days(count * 365)), // Approximate
        _ => Err(format!("Unknown time unit: '{}'", unit)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_parse_iso_date() {
        // Test date only
        let result = parse_date("2024-01-15").unwrap();
        assert!(result < SystemTime::now());

        // Test datetime with timezone
        let result = parse_date("2024-01-15T10:30:00Z").unwrap();
        assert!(result < SystemTime::now());

        // Test datetime with offset
        let result = parse_date("2024-01-15T10:30:00+02:00").unwrap();
        assert!(result < SystemTime::now());
    }

    #[test]
    fn test_parse_relative_dates() {
        // Test "now"
        let result = parse_date("now").unwrap();
        let diff = SystemTime::now().duration_since(result).unwrap();
        assert!(diff < StdDuration::from_secs(1));

        // Test "today"
        let result = parse_date("today").unwrap();
        let diff = SystemTime::now().duration_since(result).unwrap();
        assert!(diff < StdDuration::from_secs(1));

        // Test "yesterday"
        let result = parse_date("yesterday").unwrap();
        let diff = SystemTime::now().duration_since(result).unwrap();
        assert!(diff > StdDuration::from_secs(23 * 3600));
        assert!(diff < StdDuration::from_secs(25 * 3600));

        // Test "N days ago"
        let result = parse_date("7 days ago").unwrap();
        let diff = SystemTime::now().duration_since(result).unwrap();
        assert!(diff > StdDuration::from_secs(6 * 24 * 3600));
        assert!(diff < StdDuration::from_secs(8 * 24 * 3600));

        // Test "N weeks ago"
        let result = parse_date("2 weeks ago").unwrap();
        let diff = SystemTime::now().duration_since(result).unwrap();
        assert!(diff > StdDuration::from_secs(13 * 24 * 3600));
        assert!(diff < StdDuration::from_secs(15 * 24 * 3600));

        // Test "N months ago"
        let result = parse_date("3 months ago").unwrap();
        let diff = SystemTime::now().duration_since(result).unwrap();
        assert!(diff > StdDuration::from_secs(89 * 24 * 3600));
        assert!(diff < StdDuration::from_secs(91 * 24 * 3600));
    }

    #[test]
    fn test_parse_various_units() {
        assert!(parse_date("5 seconds ago").is_ok());
        assert!(parse_date("10 minutes ago").is_ok());
        assert!(parse_date("2 hours ago").is_ok());
        assert!(parse_date("1 day ago").is_ok());
        assert!(parse_date("1 week ago").is_ok());
        assert!(parse_date("6 months ago").is_ok());
        assert!(parse_date("1 year ago").is_ok());
    }

    #[test]
    fn test_parse_future_dates() {
        // Test "in N unit" format
        let result = parse_date("in 1 day").unwrap();
        let diff = result.duration_since(SystemTime::now()).unwrap();
        assert!(diff > StdDuration::from_secs(23 * 3600));
        assert!(diff < StdDuration::from_secs(25 * 3600));

        // Test "N unit from now" format
        let result = parse_date("2 hours from now").unwrap();
        let diff = result.duration_since(SystemTime::now()).unwrap();
        assert!(diff > StdDuration::from_secs(1 * 3600 + 50 * 60)); // 1h 50m
        assert!(diff < StdDuration::from_secs(2 * 3600 + 10 * 60)); // 2h 10m

        // Test various future units
        assert!(parse_date("in 5 minutes").is_ok());
        assert!(parse_date("3 days from now").is_ok());
        assert!(parse_date("in 1 week").is_ok());
        assert!(parse_date("2 months from now").is_ok());
    }

    #[test]
    fn test_invalid_dates() {
        assert!(parse_date("invalid").is_err());
        assert!(parse_date("2024-13-01").is_err());
        assert!(parse_date("not a date").is_err());
        assert!(parse_date("5 decades ago").is_err()); // Unknown unit
    }
}
