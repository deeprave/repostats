//! Type definitions for the queue system
//!
//! This module contains the core data structures used throughout
//! the queue system for statistics, memory management, and consumer tracking.

/// Memory usage statistics for the queue system
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryStats {
    /// Total number of messages in the queue
    pub total_messages: usize,
    /// Total memory usage in bytes
    pub total_bytes: usize,
    /// Memory used by message data
    pub message_data_bytes: usize,
    /// Memory used by Arc and metadata overhead
    pub overhead_bytes: usize,
}

/// Consumer lag statistics for the queue system
#[derive(Debug, Clone, PartialEq)]
pub struct LagStats {
    /// Total number of active consumers
    pub total_consumers: usize,
    /// Maximum lag among all consumers
    pub max_lag: usize,
    /// Minimum lag among all consumers
    pub min_lag: usize,
    /// Average lag across all consumers
    pub avg_lag: f64,
}

/// Information about a stale consumer
#[derive(Debug, Clone)]
pub struct StaleConsumerInfo {
    /// Internal consumer ID
    pub consumer_id: u64,
    /// Current lag in messages
    pub lag: usize,
    /// Time since last read (seconds)
    pub seconds_since_last_read: u64,
}
