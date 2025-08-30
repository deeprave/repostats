//! Scanner API
//!
//! This module provides the public API for the scanner system, consolidating all external
//! exports and providing a controlled interface for accessing scanner functionality.
//!
//! This follows the same pattern as the plugin::api and queue::api modules to maintain
//! consistent architecture across the application.

// Core scanner management
pub use crate::scanner::manager::ScannerManager;

// Error handling
pub use crate::scanner::error::{ScanError, ScanResult};

// Scanner task functionality
pub use crate::scanner::task::ScannerTask;

// Core data types and structures
pub use crate::scanner::types::{
    ChangeType, CommitInfo, FileChangeData, RepositoryData, RepositoryDataBuilder, ScanMessage,
    ScanRequires, ScanStats,
};
