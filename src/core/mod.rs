//! Core services and infrastructure

pub mod cleanup;
pub mod error_handling;
pub mod logging;
pub mod pattern_parser;
pub mod query;
pub mod retry;
pub mod services;
pub mod shutdown;
pub mod strings;
pub mod styles;
pub mod sync;
pub mod time;
pub mod validation;
pub mod version; // centralized styling palette for CLI & plugins

#[cfg(test)]
mod tests;
