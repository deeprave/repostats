//! Plugin System Module
//!
//! Provides a trait-based interface for plugin communication with async notifications.
//! Supports dynamic plugin loading, version compatibility, and real-time notifications.

// Internal modules - all access should go through api module
pub(crate) mod activation;
pub(crate) mod args;
pub(crate) mod builtin;
pub(crate) mod controller;
pub(crate) mod data_export;
pub(crate) mod discovery;
pub(crate) mod error;
pub(crate) mod error_handling;
pub(crate) mod events;
pub(crate) mod external;
pub(crate) mod initialization;
pub(crate) mod manager;
pub(crate) mod registry;
pub(crate) mod traits;
pub(crate) mod types;

// Public API module - the only public interface for the plugin system
pub mod api;

#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod tests;
