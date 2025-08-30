// Internal modules - all access should go through api module
pub(crate) mod error;
pub(crate) mod event;
pub(crate) mod manager;
pub(crate) mod traits;

// Public API module - the only public interface for the notification system
pub mod api;

#[cfg(test)]
mod tests;
