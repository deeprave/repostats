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
    /// Git operation failed
    Git { message: String },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Repository { message } => write!(f, "Repository error: {}", message),
            ScanError::Io { message } => write!(f, "IO error: {}", message),
            ScanError::Configuration { message } => write!(f, "Configuration error: {}", message),
            ScanError::Git { message } => write!(f, "Git error: {}", message),
        }
    }
}

impl std::error::Error for ScanError {}

pub type ScanResult<T> = Result<T, ScanError>;
