//! Tests for date parsing utilities
//!
//! This module contains all tests for the date_parser module, including
//! ISO 8601 date parsing, relative date parsing, and error handling.

use crate::app::cli::date_parser::*;
use std::time::{Duration as StdDuration, SystemTime};

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
