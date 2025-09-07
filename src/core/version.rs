//! Build metadata and API version accessors shared across app and plugins.
//! This includes the generated version.rs from the build script into a core module,
//! providing a single source of truth.

include!(concat!(env!("OUT_DIR"), "/version.rs"));

/// Parse the API version string from build script into u32.
/// Falls back to a stable default if parsing fails.
pub fn get_api_version() -> u32 {
    PLUGIN_API_VERSION.parse().unwrap_or(20250727)
}

/// Build time string from the build script (UTC)
pub fn build_time() -> &'static str {
    BUILD_TIME
}

/// Short git hash captured by the build script
pub fn git_hash() -> &'static str {
    GIT_HASH
}
