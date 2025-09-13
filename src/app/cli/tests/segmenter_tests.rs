//! Tests for command segmentation functionality
//!
//! This module contains all tests for the command_segmenter module, including
//! command boundary detection, argument parsing, and edge cases.

use crate::app::cli::segmenter::*;

#[test]
fn test_segment_commands_only() {
    let segmenter = CommandSegmenter::with_commands(vec!["scan".to_string(), "status".to_string()]);
    let args = vec![
        "scan".to_string(),
        "--since".to_string(),
        "1week".to_string(),
        "status".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];

    let result = segmenter.segment_commands(&args).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].command_name, "scan");
    assert_eq!(result[0].args, vec!["scan", "--since", "1week"]);
    assert_eq!(result[1].command_name, "status");
    assert_eq!(result[1].args, vec!["status", "--format", "json"]);
}

#[test]
fn test_segment_commands_only_no_commands() {
    let segmenter = CommandSegmenter::with_commands(vec!["test".to_string()]);
    let args = vec![];

    let result = segmenter.segment_commands(&args).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_segment_commands_only_single_command() {
    let segmenter = CommandSegmenter::with_commands(vec!["dump".to_string()]);
    let args = vec!["dump".to_string(), "--verbose".to_string()];

    let result = segmenter.segment_commands(&args).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].command_name, "dump");
    assert_eq!(result[0].args, vec!["dump", "--verbose"]);
}
