//! Generic Cleanup Interface
//!
//! Provides a generic, reusable trait for cleanup operations across the system.
//! This opaque interface allows components to trigger cleanup operations without
//! needing knowledge of internal implementation details.

/// Generic trait for cleanup operations
///
/// This trait provides a clean interface for cleanup coordination that
/// maintains architectural boundaries while enabling coordinated operations.
pub trait Cleanup {
    /// Clean up all resources managed by this instance
    fn cleanup(&self);
}
