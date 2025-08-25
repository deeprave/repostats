//! Scanner Component
//!
//! This module provides a comprehensive Scanner component that manages multiple repository
//! scanner tasks, each with unique identification and queue integration. Based on gstats
//! scanner architecture but adapted for repostats' detached, loosely-coupled design.
//!
//! ## Core Features
//!
//! - **ScannerManager**: Central coordination for multiple repositories
//! - **SHA256-Based Deduplication**: Prevents scanning same repository multiple times
//! - **Remote Repository Support**: Full support for local and remote repositories via gix
//! - **Flexible Start Points**: Scan from any commit/branch/tag with content reconstruction
//! - **Comprehensive Filtering**: gstats-compatible filtering system
//! - **Plugin Integration**: Conditional file checkout with temporary directory management
//! - **Event Coordination**: Lifecycle events via notification system

pub mod error;
pub mod manager;

pub use error::{ScanError, ScanResult};
pub use manager::{
    ChangeType, CommitInfo, FileChangeData, ScanMessage, ScanStats, ScannerManager, ScannerTask,
};
