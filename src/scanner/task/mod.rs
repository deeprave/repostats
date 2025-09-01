//! Scanner Task Module
//!
//! Individual scanner task for a specific repository with scanning operations,
//! queue publishing, and event handling functionality. Split into logical submodules.

mod core;
mod events;
mod git_ops;
mod queue_ops;

#[cfg(test)]
mod tests;

pub use core::ScannerTask;
