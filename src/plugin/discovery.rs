//! Plugin discovery system
//!
//! This module provides plugin discovery functionality, delegating to the
//! unified discovery system while maintaining API consistency.

pub use crate::plugin::types::{DiscoveredPlugin, PluginSource};
pub use crate::plugin::unified_discovery::PluginDiscovery;
