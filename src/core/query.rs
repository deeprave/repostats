//! Query Parameters for Repository Filtering
//!
//! Core query parameter structures moved from scanner module to reduce coupling.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;
use thiserror::Error;

/// Query parameters for repository scanning
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct QueryParams {
    /// Date range for filtering
    pub date_range: Option<DateRange>,
    /// File path filters
    pub file_paths: FilePathFilter,
    /// Maximum number of commits to analyze
    pub max_commits: Option<usize>,
    /// Author filters
    pub authors: AuthorFilter,
    /// Git reference to scan (branch, tag, commit SHA, or HEAD)
    pub git_ref: Option<String>,
}

/// Date range specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DateRange {
    /// Start date (inclusive)
    pub start: Option<SystemTime>,
    /// End date (inclusive)
    pub end: Option<SystemTime>,
}

impl DateRange {
    /// Create a new date range with both start and end dates
    pub fn new(start: SystemTime, end: SystemTime) -> Self {
        Self {
            start: Some(start),
            end: Some(end),
        }
    }

    /// Create a date range starting from a specific date (no end date)
    pub fn from(start: SystemTime) -> Self {
        Self {
            start: Some(start),
            end: None,
        }
    }

    /// Create a date range ending at a specific date (no start date)
    pub fn until(end: SystemTime) -> Self {
        Self {
            start: None,
            end: Some(end),
        }
    }

    /// Check if this date range has both start and end dates
    pub fn is_bounded(&self) -> bool {
        self.start.is_some() && self.end.is_some()
    }

    /// Check if a given time falls within this date range
    pub fn contains(&self, time: SystemTime) -> bool {
        let after_start = self.start.is_none_or(|start| time >= start);
        let before_end = self.end.is_none_or(|end| time <= end);
        after_start && before_end
    }
}

/// File path filtering configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FilePathFilter {
    /// Paths to include (empty means include all)
    pub include: Vec<PathBuf>,
    /// Paths to exclude
    pub exclude: Vec<PathBuf>,
}

/// Author filtering configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AuthorFilter {
    /// Authors to include (empty means include all)
    pub include: Vec<String>,
    /// Authors to exclude
    pub exclude: Vec<String>,
}

/// Query parameter validation errors
#[derive(Error, Debug, PartialEq)]
pub enum QueryValidationError {
    #[error("Invalid date range: start date {start:?} is after end date {end:?}")]
    InvalidDateRange { start: SystemTime, end: SystemTime },
    #[error("Invalid max_commits: {max_commits} must be greater than 0")]
    InvalidMaxCommits { max_commits: usize },
    #[error("Empty file path provided")]
    EmptyFilePath,
    #[error("Empty author name provided")]
    EmptyAuthor,
    #[error("Empty git reference provided")]
    EmptyGitRef,
}

impl QueryParams {
    /// Create a new empty query parameters instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if this query has any date constraints
    pub fn has_date_filter(&self) -> bool {
        self.date_range.is_some()
    }

    /// Check if this query has a max_commits constraint
    pub fn has_max_commits(&self) -> bool {
        self.max_commits.is_some()
    }

    /// Get the effective max_commits, returning None if not set
    pub fn effective_max_commits(&self) -> Option<usize> {
        self.max_commits
    }

    /// Check if this query has a git reference constraint
    pub fn has_git_ref(&self) -> bool {
        self.git_ref.is_some()
    }

    /// Get the effective git reference, returning None if not set
    pub fn effective_git_ref(&self) -> Option<&str> {
        self.git_ref.as_deref()
    }

    /// Validate query parameters for consistency
    pub fn validate(&self) -> Result<(), QueryValidationError> {
        // Validate date range if present
        if let Some(date_range) = &self.date_range {
            if let (Some(start), Some(end)) = (date_range.start, date_range.end) {
                if start > end {
                    return Err(QueryValidationError::InvalidDateRange { start, end });
                }
            }
        }

        // Validate max_commits if present
        if let Some(max_commits) = self.max_commits {
            if max_commits == 0 {
                return Err(QueryValidationError::InvalidMaxCommits { max_commits });
            }
        }

        // Validate file paths
        for path in &self.file_paths.include {
            if path.as_os_str().is_empty() {
                return Err(QueryValidationError::EmptyFilePath);
            }
        }
        for path in &self.file_paths.exclude {
            if path.as_os_str().is_empty() {
                return Err(QueryValidationError::EmptyFilePath);
            }
        }

        // Validate authors
        for author in &self.authors.include {
            if author.is_empty() {
                return Err(QueryValidationError::EmptyAuthor);
            }
        }
        for author in &self.authors.exclude {
            if author.is_empty() {
                return Err(QueryValidationError::EmptyAuthor);
            }
        }

        // Validate git reference
        if let Some(ref git_ref) = self.git_ref {
            if git_ref.is_empty() {
                return Err(QueryValidationError::EmptyGitRef);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn test_default_query_params() {
        let params = QueryParams::default();
        assert!(params.date_range.is_none());
        assert!(params.file_paths.include.is_empty());
        assert!(params.file_paths.exclude.is_empty());
        assert!(params.max_commits.is_none());
        assert!(params.authors.include.is_empty());
        assert!(params.authors.exclude.is_empty());
        assert!(params.git_ref.is_none());
    }

    #[test]
    fn test_query_params_validation_direct() {
        let valid_params = QueryParams::default();
        assert!(valid_params.validate().is_ok());

        let start_time = UNIX_EPOCH + Duration::from_secs(1000);
        let end_time = UNIX_EPOCH + Duration::from_secs(2000);
        let valid_range = QueryParams {
            date_range: Some(DateRange {
                start: Some(start_time),
                end: Some(end_time),
            }),
            ..Default::default()
        };
        assert!(valid_range.validate().is_ok());
    }

    #[test]
    fn test_date_range_convenience_methods() {
        let start_time = UNIX_EPOCH + Duration::from_secs(1000);
        let end_time = UNIX_EPOCH + Duration::from_secs(2000);
        let middle_time = UNIX_EPOCH + Duration::from_secs(1500);
        let before_time = UNIX_EPOCH + Duration::from_secs(500);
        let after_time = UNIX_EPOCH + Duration::from_secs(2500);

        let bounded_range = DateRange::new(start_time, end_time);
        assert!(bounded_range.is_bounded());
        assert!(bounded_range.contains(start_time));
        assert!(bounded_range.contains(middle_time));
        assert!(bounded_range.contains(end_time));
        assert!(!bounded_range.contains(before_time));
        assert!(!bounded_range.contains(after_time));

        let unbounded_start = DateRange::until(end_time);
        assert!(!unbounded_start.is_bounded());
        assert!(unbounded_start.contains(before_time));
        assert!(unbounded_start.contains(start_time));
        assert!(unbounded_start.contains(end_time));
        assert!(!unbounded_start.contains(after_time));

        let unbounded_end = DateRange::from(start_time);
        assert!(!unbounded_end.is_bounded());
        assert!(!unbounded_end.contains(before_time));
        assert!(unbounded_end.contains(start_time));
        assert!(unbounded_end.contains(middle_time));
        assert!(unbounded_end.contains(after_time));
    }

    #[test]
    fn test_query_params_with_git_ref_field() {
        let mut params = QueryParams::default();
        assert!(params.git_ref.is_none());

        params.git_ref = Some("develop".to_string());
        assert_eq!(params.git_ref, Some("develop".to_string()));
    }

    #[test]
    fn test_git_ref_validation() {
        let params = QueryParams {
            git_ref: Some("".to_string()),
            ..Default::default()
        };

        let result = params.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), QueryValidationError::EmptyGitRef);
    }

    #[test]
    fn test_query_params_git_ref_convenience_methods() {
        let params_without_ref = QueryParams::default();
        assert!(!params_without_ref.has_git_ref());
        assert_eq!(params_without_ref.effective_git_ref(), None);

        let params_with_ref = QueryParams {
            git_ref: Some("main".to_string()),
            ..Default::default()
        };
        assert!(params_with_ref.has_git_ref());
        assert_eq!(params_with_ref.effective_git_ref(), Some("main"));
    }
}
