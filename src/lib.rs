pub mod app;
pub mod core;
pub mod notifications;
pub mod plugin;
pub mod queue;
pub mod scanner;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

/// Parse the API version string from build script into u32
pub fn get_plugin_api_version() -> u32 {
    PLUGIN_API_VERSION.parse().unwrap_or(20250727)
}
