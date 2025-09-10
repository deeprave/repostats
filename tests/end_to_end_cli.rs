//! CLI Integration Tests
//!
//! End-to-end CLI integration tests have been organized into focused modules
//! for better maintainability and readability.
//!
//! Tests are organized by functionality:
//! - `cli::argument_parsing` - Core CLI argument parsing tests
//! - `cli::repository_handling` - Repository argument and TOML configuration
//! - `cli::filtering` - Date, author, and file filtering tests
//! - `cli::path_handling` - Path deduplication and validation tests
//! - `cli::plugin_config` - Plugin exclusion and configuration tests
//! - `cli::checkout` - Checkout functionality tests
//! - `cli::validation` - Argument validation and error handling tests
//! - `cli::toml_config` - TOML configuration and field type mapping tests

mod cli;

// Re-export modules for convenience
pub use cli::*;
