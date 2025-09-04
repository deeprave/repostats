//! Service Registry Re-exports
//!
//! Re-exports service access functions from their respective modules.
//! All services follow SOLID principles by being located in their domain modules.

pub use crate::notifications::api::get_notification_service;
pub use crate::plugin::api::get_plugin_service;
pub use crate::queue::api::get_queue_service;
