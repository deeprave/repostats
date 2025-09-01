//! Core services and infrastructure

pub mod error_handling;
pub mod logging;
pub mod pattern_parser;
pub mod query;
pub mod services;
pub mod strings;
pub mod time;
pub mod validation;

#[cfg(test)]
mod tests;
