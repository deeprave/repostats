//! Scanner Task Module
//!
//! Individual scanner task for a specific repository with scanning operations,
//! queue publishing, and event handling functionality. Split into logical submodules.

mod core;
mod events;
mod git_ops;
mod queue_ops;

pub use core::ScannerTask;

// Re-export for backwards compatibility if needed
pub use crate::scanner::types::{
    ChangeType, CommitInfo, FileChangeData, RepositoryData, RepositoryDataBuilder, ScanMessage,
    ScanStats,
};
