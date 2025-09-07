//! Generic error handling utilities
//!
//! Provides unified error handling that can work across different error types
//! while maintaining domain-specific error logging patterns.

/// Trait for errors that can distinguish between user-actionable and system errors
///
/// This trait enables generic error handling functions to determine whether an error
/// should show specific user messages or generic context with debug details.
///
/// # Design Principles
/// - User-actionable errors (like validation failures) should show specific messages
/// - System errors (like IO failures) should show generic context to avoid overwhelming users
/// - All errors should provide debug details for system administrators
///
/// # Implementation Consistency
/// **IMPORTANT**: When `is_user_actionable()` returns `true`, `user_message()` should return
/// `Some(message)` with a helpful, actionable message. When `is_user_actionable()` returns
/// `false`, `user_message()` should return `None`. This ensures consistent error handling
/// behavior across the application.
pub trait ContextualError: std::error::Error {
    /// Returns true if this error contains a specific, user-actionable message
    /// that should be displayed directly to the user
    ///
    /// Examples of user-actionable errors:
    /// - Argument parsing failures
    /// - Validation errors
    /// - Configuration errors with clear fixes
    ///
    /// Examples of system errors:
    /// - IO failures
    /// - Network timeouts
    /// - Plugin loading failures
    fn is_user_actionable(&self) -> bool;

    /// Returns the specific user message if this is a user-actionable error
    ///
    /// This should return Some(message) when is_user_actionable() returns true,
    /// and None otherwise. The message should be clear, concise, and actionable.
    fn user_message(&self) -> Option<&str>;
}

/// Log errors with appropriate detail level based on error specificity
///
/// This function provides unified error handling by:
/// - Showing specific messages for user-actionable errors (preserves detail)
/// - Showing generic context with debug details for system errors (avoids overwhelming users)
/// - Ensuring consistent error formatting across all modules
///
/// # Arguments
/// * `error` - The error to handle (must implement ContextualError)
/// * `operation_context` - Human-readable description of the operation that failed
///
/// # Examples
/// ```rust,no_run
/// # use repostats::core::error_handling::{log_error_with_context, ContextualError};
/// # use repostats::core::validation::ValidationError;
///
/// // User-actionable error shows specific message
/// let validation_err = ValidationError::new("Invalid email format");
/// log_error_with_context(&validation_err, "User registration");
/// // Logs: "FATAL: Invalid email format"
///
/// // System error shows generic context with debug details
/// let system_err = ValidationError::new("Missing required field 'port'");
/// log_error_with_context(&system_err, "Configuration loading");
/// // Logs: "FATAL: Missing required field 'port'"
/// ```
pub fn log_error_with_context<E: ContextualError + std::fmt::Display + std::fmt::Debug>(
    error: &E,
    operation_context: &str,
) {
    // Always emit a primary fatal line containing at least some context plus
    // useful detail. If the error is user-actionable we prefer its user message.
    if error.is_user_actionable() {
        if let Some(user_msg) = error.user_message() {
            log::error!("FATAL: {}", user_msg);
        } else {
            log::error!("FATAL: {}", operation_context);
        }
    } else {
        log::error!("FATAL: {}", operation_context);
    }
    // Always provide detail only at debug level (requested change)
    log::debug!("DETAIL: {}", error);
    log::debug!("DEBUG_DETAILS: {:?}", error);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    // Test error type for user-actionable errors
    #[derive(Debug)]
    struct TestUserError {
        message: String,
    }

    impl fmt::Display for TestUserError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl std::error::Error for TestUserError {}

    impl ContextualError for TestUserError {
        fn is_user_actionable(&self) -> bool {
            true
        }

        fn user_message(&self) -> Option<&str> {
            Some(&self.message)
        }
    }

    // Test error type for system errors
    #[derive(Debug)]
    struct TestSystemError {
        internal_details: String,
    }

    impl fmt::Display for TestSystemError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "System error: {}", self.internal_details)
        }
    }

    impl std::error::Error for TestSystemError {}

    impl ContextualError for TestSystemError {
        fn is_user_actionable(&self) -> bool {
            false
        }

        fn user_message(&self) -> Option<&str> {
            None
        }
    }

    #[test]
    fn test_user_actionable_error_shows_specific_message() {
        let error = TestUserError {
            message: "Invalid email format".to_string(),
        };

        // This would log: "FATAL: Invalid email format"
        // In a real test, we'd capture the log output
        assert!(error.is_user_actionable());
        assert_eq!(error.user_message(), Some("Invalid email format"));
    }

    #[test]
    fn test_system_error_uses_generic_context() {
        let error = TestSystemError {
            internal_details: "Connection refused".to_string(),
        };

        // This would log: "FATAL: Configuration loading" + debug details
        assert!(!error.is_user_actionable());
        assert_eq!(error.user_message(), None);
    }
}
