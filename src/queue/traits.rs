//! Traits for the queue system
//!
//! This module contains the trait definitions that provide extensibility
//! and customization points for the queue system.

/// Trait for messages that can be grouped together for batch processing
///
/// This trait enables sophisticated message batching scenarios where related
/// messages need to be processed together. It provides flexibility for both
/// count-based grouping (where the group size is known upfront) and
/// signal-based grouping (where completion is indicated by a special message).
///
/// # Example Implementation
///
/// ```rust,no_run
/// use repostats::queue::GroupedMessage;
///
/// struct BatchMessage {
///     batch_id: String,
///     is_first: bool,
///     is_last: bool,
///     total_count: Option<usize>,
/// }
///
/// impl GroupedMessage for BatchMessage {
///     fn group_id(&self) -> Option<String> {
///         Some(self.batch_id.clone())
///     }
///
///     fn starts_group(&self) -> Option<(String, usize)> {
///         if self.is_first {
///             self.total_count.map(|count| (self.batch_id.clone(), count))
///         } else {
///             None
///         }
///     }
///
///     fn completes_group(&self) -> Option<String> {
///         if self.is_last { Some(self.batch_id.clone()) } else { None }
///     }
/// }
/// ```
pub trait GroupedMessage {
    /// Get the group identifier for this message
    ///
    /// Returns `Some(group_id)` if this message belongs to a group,
    /// or `None` for standalone messages.
    fn group_id(&self) -> Option<String>;

    /// Check if this message starts a new group
    ///
    /// Returns `Some((group_id, expected_count))` if this message
    /// starts a new group with a known message count. This enables
    /// precise batching without waiting for completion signals.
    fn starts_group(&self) -> Option<(String, usize)>;

    /// Check if this message completes a group
    ///
    /// Returns `Some(group_id)` if this message completes a group.
    /// This is a fallback mechanism for groups without known counts.
    fn completes_group(&self) -> Option<String>;
}
