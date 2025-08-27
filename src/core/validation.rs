//! Validation utilities for CLI arguments
//!
//! Provides advanced validation logic for filter combinations and argument values.

use crate::app::cli::date_parser;

/// Validate date range arguments (since/until can now handle both ISO 8601 and relative formats)
pub fn validate_date_range(since: Option<&str>, until: Option<&str>) -> Result<(), String> {
    if let (Some(since_str), Some(until_str)) = (since, until) {
        let start_time = date_parser::parse_date(since_str)?;
        let end_time = date_parser::parse_date(until_str)?;

        if start_time > end_time {
            return Err(format!(
                "Start date '{}' (--since) is after end date '{}' (--until)",
                since_str, until_str
            ));
        }
    }
    Ok(())
}

// Reference conflict validation is no longer needed since we only have --ref

/// Validate positive integer value
pub fn validate_positive_int(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(0) => Err("Value must be greater than 0".to_string()),
        Ok(n) => Ok(n),
        Err(_) => Err(format!("'{}' is not a valid positive integer", value)),
    }
}

/// Validate file extension format
pub fn validate_extension(ext: &str) -> Result<String, String> {
    // Remove leading dot if present
    let cleaned = if ext.starts_with('.') { &ext[1..] } else { ext };

    // Check for invalid characters
    if cleaned.is_empty() {
        return Err("Extension cannot be empty".to_string());
    }

    if cleaned.contains('/') || cleaned.contains('\\') {
        return Err("Extension cannot contain path separators".to_string());
    }

    Ok(cleaned.to_lowercase())
}

/// Validate glob pattern syntax
pub fn validate_glob_pattern(pattern: &str) -> Result<String, String> {
    // Try to compile as glob pattern
    match glob::Pattern::new(pattern) {
        Ok(_) => Ok(pattern.to_string()),
        Err(e) => Err(format!("Invalid glob pattern '{}': {}", pattern, e)),
    }
}

/// Validate filter combinations for logical consistency
pub fn validate_filter_combinations(
    include_patterns: &[String],
    exclude_patterns: &[String],
    include_extensions: &[String],
    exclude_extensions: &[String],
) -> Result<(), String> {
    // Check for overlapping include/exclude extensions
    for inc_ext in include_extensions {
        if exclude_extensions.contains(inc_ext) {
            return Err(format!(
                "Extension '{}' is both included and excluded. \
                 Remove it from one of the lists.",
                inc_ext
            ));
        }
    }

    // Check for identical include/exclude patterns
    for pattern in include_patterns {
        if exclude_patterns.contains(pattern) {
            return Err(format!(
                "Pattern '{}' is both included and excluded. \
                 Remove it from one of the lists.",
                pattern
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_date_range() {
        assert!(validate_date_range(None, None).is_ok());
        assert!(validate_date_range(Some("2024-01-01"), Some("2024-12-31")).is_ok());
        assert!(validate_date_range(Some("2024-12-31"), Some("2024-01-01")).is_err());
        assert!(validate_date_range(Some("yesterday"), Some("today")).is_ok());
        assert!(validate_date_range(Some("today"), Some("yesterday")).is_err());
    }

    #[test]
    fn test_validate_positive_int() {
        assert_eq!(validate_positive_int("5").unwrap(), 5);
        assert_eq!(validate_positive_int("100").unwrap(), 100);
        assert!(validate_positive_int("0").is_err());
        assert!(validate_positive_int("-5").is_err());
        assert!(validate_positive_int("not_a_number").is_err());
    }

    #[test]
    fn test_validate_extension() {
        assert_eq!(validate_extension("rs").unwrap(), "rs");
        assert_eq!(validate_extension(".rs").unwrap(), "rs");
        assert_eq!(validate_extension("RS").unwrap(), "rs");
        assert!(validate_extension("").is_err());
        assert!(validate_extension(".").is_err());
        assert!(validate_extension("rs/toml").is_err());
    }

    #[test]
    fn test_validate_glob_pattern() {
        assert!(validate_glob_pattern("*.rs").is_ok());
        assert!(validate_glob_pattern("src/**/*.rs").is_ok());
        assert!(validate_glob_pattern("[").is_err());
    }

    #[test]
    fn test_validate_filter_combinations() {
        assert!(validate_filter_combinations(&[], &[], &[], &[]).is_ok());

        assert!(validate_filter_combinations(
            &["*.rs".to_string()],
            &["*.toml".to_string()],
            &[],
            &[]
        )
        .is_ok());

        assert!(validate_filter_combinations(
            &["*.rs".to_string()],
            &["*.rs".to_string()],
            &[],
            &[]
        )
        .is_err());

        assert!(
            validate_filter_combinations(&[], &[], &["rs".to_string()], &["rs".to_string()])
                .is_err()
        );
    }
}
