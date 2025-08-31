//! Pattern parsing utilities for file filtering
//!
//! Provides glob pattern matching and path filtering capabilities.

use glob::{Pattern, PatternError};
use std::path::{Path, PathBuf};

/// File pattern matcher for include/exclude filtering
#[derive(Debug, Clone)]
pub struct FilePatternMatcher {
    include_patterns: Vec<Pattern>,
    exclude_patterns: Vec<Pattern>,
    include_extensions: Vec<String>,
    exclude_extensions: Vec<String>,
    include_paths: Vec<PathBuf>,
}

impl FilePatternMatcher {
    /// Create a new file pattern matcher from CLI arguments
    pub fn new(
        include_patterns: &[String],
        exclude_patterns: &[String],
        include_extensions: &[String],
        exclude_extensions: &[String],
        include_paths: &[String],
    ) -> Result<Self, String> {
        let include_patterns = parse_patterns(include_patterns)?;
        let exclude_patterns = parse_patterns(exclude_patterns)?;

        let include_extensions = include_extensions
            .iter()
            .map(|s| s.to_lowercase())
            .collect();

        let exclude_extensions = exclude_extensions
            .iter()
            .map(|s| s.to_lowercase())
            .collect();

        let include_paths = include_paths.iter().map(PathBuf::from).collect();

        Ok(Self {
            include_patterns,
            exclude_patterns,
            include_extensions,
            exclude_extensions,
            include_paths,
        })
    }

    /// Check if a file path matches the filter criteria.
    ///
    /// Files matching any exclude pattern or extension are always excluded,
    /// even if they also match an include pattern or extension.
    /// This means exclusion takes precedence over inclusion.
    pub fn matches(&self, path: &Path) -> bool {
        // Check excluded patterns first
        for pattern in &self.exclude_patterns {
            if pattern.matches_path(path) {
                return false;
            }
        }

        // Check excluded extensions
        if !self.exclude_extensions.is_empty() {
            if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                if self.exclude_extensions.contains(&ext_lower) {
                    return false;
                }
            }
        }

        // If no include filters specified, include by default
        let has_include_filters = !self.include_patterns.is_empty()
            || !self.include_extensions.is_empty()
            || !self.include_paths.is_empty();

        if !has_include_filters {
            return true;
        }

        // Check include patterns
        for pattern in &self.include_patterns {
            if pattern.matches_path(path) {
                return true;
            }
        }

        // Check include extensions
        if !self.include_extensions.is_empty() {
            if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                if self.include_extensions.contains(&ext_lower) {
                    return true;
                }
            }
        }

        // Check include paths
        // NOTE: only relative paths are accepted here as these are
        // paths from a git commit, and not filesystem paths.
        for include_path in &self.include_paths {
            if Self::is_path_prefix_match(path, include_path) {
                return true;
            }
        }

        false
    }

    /// Check if a path matches a directory prefix with proper boundary detection
    ///
    /// This prevents false positives like matching 'src2' when 'src' is intended.
    ///
    /// Examples:
    /// - `is_path_prefix_match("src/main.rs", "src")` -> `true`
    /// - `is_path_prefix_match("src2/main.rs", "src")` -> `false`
    /// - `is_path_prefix_match("src", "src")` -> `true`
    /// - `is_path_prefix_match("src/nested/file.rs", "src")` -> `true`
    fn is_path_prefix_match(path: &Path, prefix: &Path) -> bool {
        // Convert both paths to string representations for consistent comparison
        let path_str = path.to_string_lossy();
        let prefix_str = prefix.to_string_lossy();

        // Handle exact match
        if path_str == prefix_str {
            return true;
        }

        // For prefix matching, ensure the path starts with prefix followed by a path separator
        // This prevents 'src2' from matching when 'src' is the intended prefix
        if path_str.starts_with(&*prefix_str) {
            let prefix_len = prefix_str.len();
            // Check if we have a character after the prefix
            if path_str.len() > prefix_len {
                // Get the byte at the position after prefix
                let next_byte = path_str.as_bytes()[prefix_len];
                // Must be followed by a path separator ('/' or '\\')
                return next_byte == b'/' || next_byte == b'\\';
            }
        }

        false
    }
}

/// Parse glob patterns from strings
fn parse_patterns(pattern_strings: &[String]) -> Result<Vec<Pattern>, String> {
    pattern_strings
        .iter()
        .map(|s| {
            Pattern::new(s).map_err(|e: PatternError| format!("Invalid pattern '{}': {}", s, e))
        })
        .collect()
}

/// Parse a comma-separated list of extensions
pub fn parse_extensions(extensions_str: &str) -> Vec<String> {
    extensions_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            // Remove leading dot if present
            if s.starts_with('.') {
                s[1..].to_lowercase()
            } else {
                s.to_lowercase()
            }
        })
        .collect()
}

/// Author pattern matcher for filtering by name or email with glob patterns
#[derive(Debug, Clone)]
pub struct AuthorPatternMatcher {
    include_patterns: Vec<Pattern>,
    exclude_patterns: Vec<Pattern>,
}

impl AuthorPatternMatcher {
    /// Create a new author pattern matcher from include/exclude lists
    pub fn new(include_patterns: &[String], exclude_patterns: &[String]) -> Result<Self, String> {
        // Auto-complete incomplete email patterns and convert to lowercase
        let include_processed: Vec<String> = include_patterns
            .iter()
            .map(|p| Self::auto_complete_email_pattern(&p.to_lowercase()))
            .collect();
        let exclude_processed: Vec<String> = exclude_patterns
            .iter()
            .map(|p| Self::auto_complete_email_pattern(&p.to_lowercase()))
            .collect();

        let include_patterns = parse_patterns(&include_processed)?;
        let exclude_patterns = parse_patterns(&exclude_processed)?;

        Ok(Self {
            include_patterns,
            exclude_patterns,
        })
    }

    /// Check if an author matches the filter criteria.
    ///
    /// Authors matching any exclude pattern are always excluded,
    /// even if they also match an include pattern.
    /// This means exclusion takes precedence over inclusion.
    pub fn matches(&self, author_name: &str, author_email: &str) -> bool {
        // Check excluded patterns first
        for pattern in &self.exclude_patterns {
            if self.matches_single_pattern(pattern, author_name, author_email) {
                return false;
            }
        }

        // If no include filters specified, include by default
        if self.include_patterns.is_empty() {
            return true;
        }

        // Check include patterns
        for pattern in &self.include_patterns {
            if self.matches_single_pattern(pattern, author_name, author_email) {
                return true;
            }
        }

        false
    }

    /// Check if a single pattern matches either author name or email
    fn matches_single_pattern(
        &self,
        pattern: &Pattern,
        author_name: &str,
        author_email: &str,
    ) -> bool {
        let pattern_str = pattern.as_str();

        if pattern_str.contains('@') {
            // Email matching - case insensitive (patterns are already lowercase)
            pattern.matches(&author_email.to_lowercase())
        } else {
            // Name matching - case insensitive (patterns are already lowercase)
            pattern.matches(&author_name.to_lowercase())
        }
    }

    /// Auto-complete incomplete email patterns for improved user experience
    ///
    /// This function allows users to write simplified email patterns without explicit wildcards:
    /// - `@amazon.com` matches any user at amazon.com (auto-expands to `*@amazon.com`)
    /// - `user@` matches user at any domain (auto-expands to `user@*`)
    /// - `@` matches any email address (auto-expands to `*@*`)
    ///
    /// Complete patterns with explicit wildcards are left unchanged.
    /// Non-email patterns (no `@` symbol) are also left unchanged.
    fn auto_complete_email_pattern(pattern: &str) -> String {
        if !pattern.contains('@') {
            // Name pattern, no changes needed
            return pattern.to_string();
        }

        if pattern == "@" {
            // Special case: bare @ means match any email
            "*@*".to_string()
        } else if pattern.starts_with('@') && !pattern.starts_with("*@") {
            // Missing username part: @amazon.com → *@amazon.com
            format!("*{}", pattern)
        } else if pattern.ends_with('@') && !pattern.ends_with("@*") {
            // Missing domain part: user@ → user@*
            format!("{}*", pattern)
        } else {
            // Pattern is already complete or has explicit wildcards
            pattern.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        let matcher = FilePatternMatcher::new(
            &["src/**/*.rs".to_string()],
            &["**/test_*.rs".to_string()],
            &[],
            &[],
            &[],
        )
        .unwrap();

        assert!(matcher.matches(Path::new("src/main.rs")));
        assert!(matcher.matches(Path::new("src/lib.rs")));
        assert!(matcher.matches(Path::new("src/module/file.rs")));
        assert!(!matcher.matches(Path::new("src/test_utils.rs")));
        assert!(!matcher.matches(Path::new("tests/integration.rs")));
    }

    #[test]
    fn test_extension_filtering() {
        let matcher = FilePatternMatcher::new(
            &[],
            &[],
            &["rs".to_string(), "toml".to_string()],
            &["lock".to_string()],
            &[],
        )
        .unwrap();

        assert!(matcher.matches(Path::new("src/main.rs")));
        assert!(matcher.matches(Path::new("Cargo.toml")));
        assert!(!matcher.matches(Path::new("Cargo.lock")));
        assert!(!matcher.matches(Path::new("README.md")));
    }

    #[test]
    fn test_path_filtering() {
        let matcher = FilePatternMatcher::new(
            &[],
            &[],
            &[],
            &[],
            &["src".to_string(), "tests".to_string()],
        )
        .unwrap();

        assert!(matcher.matches(Path::new("src/main.rs")));
        assert!(matcher.matches(Path::new("tests/integration.rs")));
        assert!(!matcher.matches(Path::new("docs/README.md")));
        assert!(!matcher.matches(Path::new("Cargo.toml")));
    }

    #[test]
    fn test_path_prefix_boundary_detection() {
        let matcher = FilePatternMatcher::new(&[], &[], &[], &[], &["src".to_string()]).unwrap();

        // Should match files in the 'src' directory
        assert!(matcher.matches(Path::new("src/main.rs")));
        assert!(matcher.matches(Path::new("src/lib.rs")));
        assert!(matcher.matches(Path::new("src/module/file.rs")));

        // Should match the exact directory name
        assert!(matcher.matches(Path::new("src")));

        // Should NOT match directories that merely start with 'src'
        assert!(!matcher.matches(Path::new("src2/file.rs")));
        assert!(!matcher.matches(Path::new("src_backup/file.rs")));
        assert!(!matcher.matches(Path::new("src.old/file.rs")));

        // Should not match files that have 'src' as a substring but not a path prefix
        assert!(!matcher.matches(Path::new("lib/src_utils.rs")));
        assert!(!matcher.matches(Path::new("sources/main.rs")));
    }

    #[test]
    fn test_complex_path_prefix_scenarios() {
        let matcher = FilePatternMatcher::new(
            &[],
            &[],
            &[],
            &[],
            &["lib/crypto".to_string(), "tests/unit".to_string()],
        )
        .unwrap();

        // Should match nested path prefixes correctly
        assert!(matcher.matches(Path::new("lib/crypto/hash.rs")));
        assert!(matcher.matches(Path::new("lib/crypto/aes/cipher.rs")));
        assert!(matcher.matches(Path::new("tests/unit/parser_test.rs")));

        // Should match exact paths
        assert!(matcher.matches(Path::new("lib/crypto")));
        assert!(matcher.matches(Path::new("tests/unit")));

        // Should NOT match paths that start with the prefix but aren't true prefixes
        assert!(!matcher.matches(Path::new("lib/cryptography/rsa.rs")));
        assert!(!matcher.matches(Path::new("lib/crypto_old/legacy.rs")));
        assert!(!matcher.matches(Path::new("tests/unittest/helper.rs")));
        assert!(!matcher.matches(Path::new("tests/unit_helper/util.rs")));
    }

    #[test]
    fn test_edge_cases_path_matching() {
        let matcher =
            FilePatternMatcher::new(&[], &[], &[], &[], &["a".to_string(), "ab".to_string()])
                .unwrap();

        // Test very short path prefixes
        assert!(matcher.matches(Path::new("a/file.rs")));
        assert!(matcher.matches(Path::new("ab/file.rs")));
        assert!(matcher.matches(Path::new("a")));
        assert!(matcher.matches(Path::new("ab")));

        // Should not match longer names that start with the prefix
        assert!(!matcher.matches(Path::new("abc/file.rs")));
        assert!(!matcher.matches(Path::new("a1/file.rs")));
        assert!(!matcher.matches(Path::new("ab1/file.rs")));
    }

    #[test]
    fn test_windows_path_separators() {
        let matcher = FilePatternMatcher::new(&[], &[], &[], &[], &["src".to_string()]).unwrap();

        // Test with Windows-style path separators (handled correctly)
        assert!(matcher.matches(Path::new("src\\main.rs")));
        assert!(matcher.matches(Path::new("src\\module\\file.rs")));

        // Should still prevent false positives with Windows paths
        assert!(!matcher.matches(Path::new("src2\\file.rs")));
    }

    #[test]
    fn test_is_path_prefix_match_directly() {
        use std::path::Path;

        // Test exact matches
        assert!(FilePatternMatcher::is_path_prefix_match(
            Path::new("src"),
            Path::new("src")
        ));
        assert!(FilePatternMatcher::is_path_prefix_match(
            Path::new("lib/crypto"),
            Path::new("lib/crypto")
        ));

        // Test valid prefix matches (followed by separator)
        assert!(FilePatternMatcher::is_path_prefix_match(
            Path::new("src/main.rs"),
            Path::new("src")
        ));
        assert!(FilePatternMatcher::is_path_prefix_match(
            Path::new("src/module/file.rs"),
            Path::new("src")
        ));
        assert!(FilePatternMatcher::is_path_prefix_match(
            Path::new("lib/crypto/hash.rs"),
            Path::new("lib/crypto")
        ));

        // Test invalid prefix matches (not followed by separator)
        assert!(!FilePatternMatcher::is_path_prefix_match(
            Path::new("src2/file.rs"),
            Path::new("src")
        ));
        assert!(!FilePatternMatcher::is_path_prefix_match(
            Path::new("src_backup/file.rs"),
            Path::new("src")
        ));
        assert!(!FilePatternMatcher::is_path_prefix_match(
            Path::new("src.old/file.rs"),
            Path::new("src")
        ));
        assert!(!FilePatternMatcher::is_path_prefix_match(
            Path::new("lib/cryptography/rsa.rs"),
            Path::new("lib/crypto")
        ));

        // Test cases where path is shorter than prefix
        assert!(!FilePatternMatcher::is_path_prefix_match(
            Path::new("sr"),
            Path::new("src")
        ));
        assert!(!FilePatternMatcher::is_path_prefix_match(
            Path::new("lib"),
            Path::new("lib/crypto")
        ));

        // Test with Windows path separators
        assert!(FilePatternMatcher::is_path_prefix_match(
            Path::new("src\\main.rs"),
            Path::new("src")
        ));
        assert!(!FilePatternMatcher::is_path_prefix_match(
            Path::new("src2\\file.rs"),
            Path::new("src")
        ));
    }

    #[test]
    fn test_combined_filters() {
        let matcher = FilePatternMatcher::new(
            &["**/*.rs".to_string()],
            &["**/target/**".to_string()],
            &[],
            &["bak".to_string()],
            &["src".to_string()],
        )
        .unwrap();

        assert!(matcher.matches(Path::new("src/main.rs")));
        assert!(!matcher.matches(Path::new("target/debug/main.rs")));
        assert!(!matcher.matches(Path::new("src/main.rs.bak")));
    }

    #[test]
    fn test_parse_extensions() {
        assert_eq!(parse_extensions("rs,toml,md"), vec!["rs", "toml", "md"]);
        assert_eq!(parse_extensions(".rs,.toml"), vec!["rs", "toml"]);
        assert_eq!(parse_extensions("RS,TOML"), vec!["rs", "toml"]);
        assert_eq!(parse_extensions("rs, toml , md"), vec!["rs", "toml", "md"]);
        assert_eq!(parse_extensions(""), Vec::<String>::new());
    }

    #[test]
    fn test_no_include_filters_includes_all() {
        let matcher = FilePatternMatcher::new(&[], &[], &[], &[], &[]).unwrap();

        assert!(matcher.matches(Path::new("any/file.txt")));
        assert!(matcher.matches(Path::new("src/main.rs")));
    }

    #[test]
    fn test_exclude_overrides_include() {
        let matcher = FilePatternMatcher::new(
            &["**/*.rs".to_string()],
            &["**/*.rs".to_string()],
            &[],
            &[],
            &[],
        )
        .unwrap();

        assert!(!matcher.matches(Path::new("src/main.rs")));
    }

    #[test]
    fn test_author_pattern_matching() {
        let matcher = AuthorPatternMatcher::new(
            &["David*".to_string(), "*@*amazon.*".to_string()],
            &["*@*test*".to_string()],
        )
        .unwrap();

        // Name matching (case insensitive)
        assert!(matcher.matches("David Nugent", "david@example.com"));
        assert!(matcher.matches("David L Nugent", "david@example.com"));
        assert!(matcher.matches("david nugent", "david@example.com"));
        assert!(!matcher.matches("John Smith", "john@example.com"));

        // Email matching (case insensitive) - using *@*amazon.* to match both amazon.com and aws.amazon.net
        assert!(matcher.matches("John Smith", "john@amazon.com"));
        assert!(matcher.matches("John Smith", "john@aws.amazon.net"));
        assert!(matcher.matches("John Smith", "john@Amazon.com"));
        assert!(!matcher.matches("John Smith", "john@google.com"));

        // Exclude takes precedence
        assert!(!matcher.matches("David Test", "david@test.com"));
        assert!(!matcher.matches("John", "john@amazon.testnet.com"));
    }

    #[test]
    fn test_author_pattern_no_include_filters() {
        let matcher = AuthorPatternMatcher::new(&[], &["*@*test*".to_string()]).unwrap();

        // No include filters means include all by default
        assert!(matcher.matches("Anyone", "anyone@example.com"));
        assert!(matcher.matches("John", "john@amazon.com"));

        // But excludes still apply
        assert!(!matcher.matches("David", "david@test.com"));
    }

    #[test]
    fn test_author_pattern_email_vs_name_detection() {
        let matcher =
            AuthorPatternMatcher::new(&["*@*amazon.*".to_string(), "David*".to_string()], &[])
                .unwrap();

        // Email patterns (contain @) only match email
        assert!(matcher.matches("John Smith", "john@amazon.com"));
        assert!(!matcher.matches("john@amazon.com", "john@example.com")); // Name looks like email but pattern is for email

        // Name patterns (no @) only match name
        assert!(matcher.matches("David Nugent", "david@example.com"));
        assert!(matcher.matches("John Smith", "david@amazon.com")); // Email matches (pattern is OR logic)

        // Test a case where neither email nor name match
        assert!(!matcher.matches("John Smith", "john@google.com")); // Neither email nor name match
    }

    #[test]
    fn test_author_pattern_case_insensitive() {
        let matcher =
            AuthorPatternMatcher::new(&["DAVID*".to_string(), "*@AMAZON.*".to_string()], &[])
                .unwrap();

        assert!(matcher.matches("david nugent", "david@example.com"));
        assert!(matcher.matches("David Nugent", "david@example.com"));
        assert!(matcher.matches("DAVID NUGENT", "david@example.com"));

        assert!(matcher.matches("John", "john@amazon.com"));
        assert!(matcher.matches("John", "john@AMAZON.com"));
        assert!(matcher.matches("John", "john@Amazon.Com"));
    }

    #[test]
    fn test_email_pattern_auto_completion() {
        // Test auto-completion of incomplete email patterns

        // Test 1: @domain.com should auto-expand to *@domain.com
        let matcher1 = AuthorPatternMatcher::new(&["@amazon.com".to_string()], &[]).unwrap();
        assert!(matcher1.matches("anyone", "user@amazon.com"));
        assert!(matcher1.matches("anyone", "admin@amazon.com"));
        assert!(!matcher1.matches("anyone", "user@google.com"));

        // Test 2: user@ should auto-expand to user@*
        let matcher2 = AuthorPatternMatcher::new(&["john@".to_string()], &[]).unwrap();
        assert!(matcher2.matches("anyone", "john@amazon.com"));
        assert!(matcher2.matches("anyone", "john@google.com"));
        assert!(!matcher2.matches("anyone", "jane@amazon.com"));

        // Test 3: bare @ should auto-expand to *@* (match any email)
        let matcher3 = AuthorPatternMatcher::new(&["@".to_string()], &[]).unwrap();
        assert!(matcher3.matches("anyone", "user@amazon.com"));
        assert!(matcher3.matches("anyone", "admin@google.com"));
        assert!(matcher3.matches("anyone", "test@example.org"));

        // Test 4: Existing complete patterns should be unchanged
        let matcher4 = AuthorPatternMatcher::new(&["*@amazon.com".to_string()], &[]).unwrap();
        assert!(matcher4.matches("anyone", "user@amazon.com"));
        assert!(!matcher4.matches("anyone", "user@google.com"));

        // Test 5: Mix of complete and incomplete patterns
        let matcher5 = AuthorPatternMatcher::new(
            &[
                "@amazon.com".to_string(),
                "john@".to_string(),
                "*@*.edu".to_string(),
            ],
            &[],
        )
        .unwrap();
        assert!(matcher5.matches("anyone", "user@amazon.com")); // @amazon.com → *@amazon.com
        assert!(matcher5.matches("anyone", "john@google.com")); // john@ → john@*
        assert!(matcher5.matches("anyone", "student@mit.edu")); // *@*.edu (unchanged)
        assert!(!matcher5.matches("anyone", "user@google.com")); // Doesn't match any pattern
    }
}
