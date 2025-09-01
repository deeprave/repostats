//! Scanner Error Types

use std::fmt;

/// Scanner error types
#[derive(Debug, Clone)]
pub enum ScanError {
    /// Repository validation failed
    Repository { message: String },
    /// IO operation failed
    Io { message: String },
    /// Invalid configuration
    Configuration { message: String },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Repository { message } => write!(f, "Repository error: {}", message),
            ScanError::Io { message } => write!(f, "IO error: {}", message),
            ScanError::Configuration { message } => write!(f, "Configuration error: {}", message),
        }
    }
}

impl std::error::Error for ScanError {}

impl crate::core::error_handling::ContextualError for ScanError {
    fn is_user_actionable(&self) -> bool {
        match self {
            ScanError::Configuration { .. } => true, // User can fix config issues
            ScanError::Repository { .. } => false,   // System/Git issues
            ScanError::Io { .. } => false,           // System IO issues
        }
    }

    fn user_message(&self) -> Option<&str> {
        match self {
            ScanError::Configuration { message } => Some(message),
            _ => None,
        }
    }
}

pub type ScanResult<T> = Result<T, ScanError>;
