//! Scanner Requirements Tests
//!
//! Tests for ScanRequires dependency resolution and requirement handling

use super::helpers::*;
// Removed unused import: super::super::*
use crate::scanner::types::ScanRequires;

#[test]
fn test_requirements_dependency_resolution() {
    // Test that ScanRequires correctly resolves dependencies
    let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::FILE_CONTENT);
    // FILE_CONTENT should include FILE_CHANGES and COMMITS
    let reqs = scanner.requirements();
    assert!(reqs.requires_file_content());
    assert!(reqs.requires_file_changes()); // dependency
    assert!(reqs.requires_commits()); // dependency
}

#[test]
fn test_history_requirements() {
    let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::HISTORY);
    let reqs = scanner.requirements();
    assert!(reqs.requires_history());
    assert!(reqs.requires_commits()); // dependency
                                      // History should not automatically include other requirements
    assert!(!reqs.requires_file_changes());
    assert!(!reqs.requires_file_content());
}

#[test]
fn test_combined_requirements() {
    let combined = ScanRequires::FILE_CONTENT | ScanRequires::HISTORY;
    let (_temp_dir, scanner) = create_test_scanner_task(combined);
    // Should include all specified requirements and their dependencies
    let reqs = scanner.requirements();
    assert!(reqs.requires_file_content());
    assert!(reqs.requires_file_changes()); // dependency of FILE_CONTENT
    assert!(reqs.requires_commits()); // dependency of multiple
    assert!(reqs.requires_history());
}

#[test]
fn test_no_requirements() {
    let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::NONE);
    let reqs = scanner.requirements();
    assert!(!reqs.requires_commits());
    assert!(!reqs.requires_file_changes());
    assert!(!reqs.requires_file_content());
    assert!(!reqs.requires_history());
}

#[test]
fn test_commits_only() {
    let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::COMMITS);
    let reqs = scanner.requirements();
    assert!(reqs.requires_commits());
    // COMMITS has no automatic dependencies
    assert!(!reqs.requires_file_changes());
    assert!(!reqs.requires_file_content());
    assert!(!reqs.requires_history());
}

#[test]
fn test_file_changes_includes_commits() {
    let (_temp_dir, scanner) = create_test_scanner_task(ScanRequires::FILE_CHANGES);
    let reqs = scanner.requirements();
    assert!(reqs.requires_file_changes());
    assert!(reqs.requires_commits()); // dependency
                                      // Should not include higher-level requirements
    assert!(!reqs.requires_file_content());
    assert!(!reqs.requires_history());
}

#[test]
fn test_conditional_data_collection_logic() {
    // Test that different requirement combinations trigger appropriate data collection

    // Basic requirements - should collect minimal data
    let basic_reqs = ScanRequires::COMMITS;
    let (_temp_dir, scanner_basic) = create_test_scanner_task(basic_reqs);
    let basic_requirements = scanner_basic.requirements();
    assert!(basic_requirements.requires_commits());
    assert!(!basic_requirements.requires_file_changes());

    // File change requirements - should collect commit + file change data
    let file_reqs = ScanRequires::FILE_CHANGES;
    let (_temp_dir, scanner_files) = create_test_scanner_task(file_reqs);
    let file_requirements = scanner_files.requirements();
    assert!(file_requirements.requires_commits());
    assert!(file_requirements.requires_file_changes());
    assert!(!file_requirements.requires_file_content());

    // History requirements - different data collection strategy
    let history_reqs = ScanRequires::HISTORY;
    let (_temp_dir, scanner_history) = create_test_scanner_task(history_reqs);
    let history_requirements = scanner_history.requirements();
    assert!(history_requirements.requires_history());
    assert!(history_requirements.requires_commits()); // dependency
    assert!(!history_requirements.requires_file_changes());

    // Combined requirements - should collect all requested data types
    let combined = ScanRequires::FILE_CONTENT | ScanRequires::HISTORY;
    let (_temp_dir, scanner_all) = create_test_scanner_task(combined);
    let all_reqs = scanner_all.requirements();
    assert!(all_reqs.requires_file_content());
    assert!(all_reqs.requires_file_changes()); // dependency
    assert!(all_reqs.requires_commits()); // dependency
    assert!(all_reqs.requires_history());
}

/// Test the automatic dependency inclusion in ScanRequires
#[test]
fn test_automatic_dependency_inclusion() {
    // FILE_CONTENT should automatically include FILE_CHANGES and COMMITS
    assert!(ScanRequires::FILE_CONTENT.requires_file_content());
    assert!(ScanRequires::FILE_CONTENT.requires_file_changes());
    assert!(ScanRequires::FILE_CONTENT.requires_commits());

    // FILE_CHANGES should automatically include COMMITS
    assert!(ScanRequires::FILE_CHANGES.requires_file_changes());
    assert!(ScanRequires::FILE_CHANGES.requires_commits());
    assert!(!ScanRequires::FILE_CHANGES.requires_file_content()); // should not include higher-level

    // COMMITS should be self-contained
    assert!(ScanRequires::COMMITS.requires_commits());
    assert!(!ScanRequires::COMMITS.requires_file_changes());
    assert!(!ScanRequires::COMMITS.requires_file_content());

    // HISTORY should include COMMITS
    assert!(ScanRequires::HISTORY.requires_history());
    assert!(ScanRequires::HISTORY.requires_commits()); // dependency
    assert!(!ScanRequires::HISTORY.requires_file_changes());
    assert!(!ScanRequires::HISTORY.requires_file_content());
}
