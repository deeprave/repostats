//! Tests for scanner types
//!
//! Tests for data structures used throughout the scanner system.

use crate::core::query::{AuthorFilter, DateRange, FilePathFilter, QueryParams};
use crate::scanner::types::*;
use std::path::PathBuf;
use std::time::SystemTime;

#[test]
fn test_repository_data_builder_basic() {
    let mut builder = RepositoryData::builder().with_repository("/path/to/repo");

    // Manually set git_dir since we're not using with_repository_info
    builder.git_dir = Some("/path/to/repo/.git".to_string());

    let repo_data = builder.build().expect("Should build successfully");

    assert_eq!(repo_data.path, "/path/to/repo");
    assert_eq!(repo_data.git_dir, "/path/to/repo/.git");
    assert_eq!(repo_data.is_bare, false);
    assert_eq!(repo_data.is_shallow, false);
}

#[test]
fn test_repository_data_builder_with_non_restrictive_query() {
    let query = QueryParams {
        git_ref: Some("main".to_string()),
        date_range: None, // No date restriction
        file_paths: FilePathFilter {
            include: vec![], // No file restrictions
            exclude: vec![],
        },
        authors: AuthorFilter {
            include: vec![], // No author restrictions
            exclude: vec![],
        },
        max_commits: None, // Unlimited commits
    };

    let mut builder = RepositoryData::builder()
        .with_repository("/path/to/repo")
        .with_query(&query);

    // Manually set git_dir since we're not using with_repository_info
    builder.git_dir = Some("/path/.git".to_string());

    let repo_data = builder.build().expect("Should build successfully");

    assert_eq!(repo_data.git_ref, Some("main".to_string()));
    assert_eq!(repo_data.date_range, None);
    assert_eq!(repo_data.file_paths, None);
    assert_eq!(repo_data.authors, None);
    assert_eq!(repo_data.max_commits, None);
}

#[test]
fn test_repository_data_builder_with_specified_query() {
    let query = QueryParams {
        git_ref: None,
        date_range: Some(DateRange::new(
            SystemTime::UNIX_EPOCH,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(86400),
        )),
        file_paths: FilePathFilter {
            include: vec![PathBuf::from("*.rs")],
            exclude: vec![PathBuf::from("*.tmp")],
        },
        authors: AuthorFilter {
            include: vec!["author1".to_string()],
            exclude: vec![],
        },
        max_commits: Some(100),
    };

    let mut builder = RepositoryData::builder()
        .with_repository("/path/to/repo")
        .with_query(&query);

    // Manually set git_dir since we're not using with_repository_info
    builder.git_dir = Some("/path/.git".to_string());

    let repo_data = builder.build().expect("Should build successfully");

    // Now all specified filters should be included
    assert_eq!(repo_data.file_paths, Some("*.rs".to_string()));
    assert_eq!(repo_data.authors, Some("author1".to_string()));
    assert_eq!(repo_data.max_commits, Some(100));
    assert!(repo_data.date_range.is_some());
}
