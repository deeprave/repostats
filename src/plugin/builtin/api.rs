//! API for builtin plugin registration and discovery
//!
//! This module provides the dynamic registration system for builtin plugins.
//! Plugins use the `builtin!` macro to register themselves for automatic discovery.

use crate::plugin::discovery::DiscoveredPlugin;
use inventory;

/// Entry for a builtin plugin in the dynamic registry
pub struct BuiltinPluginEntry {
    pub factory: fn() -> DiscoveredPlugin,
}

// Collect all builtin plugin entries
inventory::collect!(BuiltinPluginEntry);

/// Macro for registering builtin plugins
///
#[macro_export]
macro_rules! builtin {
    ($factory_expr:expr) => {
        inventory::submit!($crate::plugin::builtin::api::BuiltinPluginEntry {
            factory: $factory_expr
        });
    };
}

/// Get all registered builtin plugins
pub fn get_all_builtin_plugins() -> Vec<DiscoveredPlugin> {
    inventory::iter::<BuiltinPluginEntry>()
        .map(|entry| (entry.factory)())
        .collect()
}
