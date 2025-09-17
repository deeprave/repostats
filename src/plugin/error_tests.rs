//! Tests for standardized error handling with thiserror
//!
//! These tests ensure that PluginError properly implements error chaining
//! and source propagation using thiserror.

#[cfg(test)]
mod tests {
    use super::super::error::*;
    use std::error::Error;
    use std::io;

    #[test]
    fn test_plugin_error_source_chain() {
        // Test that we can properly chain errors with source
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let plugin_error = PluginError::IoError {
            operation: "read".to_string(),
            path: "/test/path".to_string(),
            source: Some(Box::new(io_error)),
        };

        // Should be able to access source error
        assert!(plugin_error.source().is_some());
        let source = plugin_error.source().unwrap();
        assert_eq!(source.to_string(), "file not found");
    }

    #[test]
    fn test_plugin_error_display_formatting() {
        let error = PluginError::ExecutionError {
            plugin_name: "test_plugin".to_string(),
            operation: "process".to_string(),
            cause: "timeout".to_string(),
        };

        let display = error.to_string();
        assert!(display.contains("test_plugin"));
        assert!(display.contains("process"));
        assert!(display.contains("timeout"));
    }

    #[test]
    fn test_error_from_notification_error() {
        use crate::notifications::error::NotificationError;

        let notif_error = NotificationError::ChannelClosed("test_channel".to_string());
        let plugin_error: PluginError = notif_error.into();

        // Should properly wrap the notification error
        match plugin_error {
            PluginError::NotificationError { .. } => (),
            _ => panic!("Expected NotificationError variant"),
        }
    }

    #[test]
    fn test_error_chain_traversal() {
        // Create a chain of errors
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let plugin_error = PluginError::IoError {
            operation: "write".to_string(),
            path: "/protected/file".to_string(),
            source: Some(Box::new(io_error)),
        };

        // Should be able to traverse the error chain
        let mut error_chain = Vec::new();
        let mut current_error: &dyn Error = &plugin_error;
        error_chain.push(current_error.to_string());

        while let Some(source) = current_error.source() {
            error_chain.push(source.to_string());
            current_error = source;
        }

        assert_eq!(error_chain.len(), 2);
        assert!(error_chain[0].contains("IO error"));
        assert_eq!(error_chain[1], "access denied");
    }

    #[test]
    fn test_system_error_compatibility() {
        use crate::core::controller::SystemError;

        // Plugin errors should be convertible to SystemError for coordination
        let plugin_error = PluginError::ExecutionError {
            plugin_name: "test".to_string(),
            operation: "shutdown".to_string(),
            cause: "timeout".to_string(),
        };

        // This conversion should exist for proper error propagation
        let system_error: SystemError = plugin_error.into();
        assert!(system_error.to_string().contains("Plugin"));
    }

    #[test]
    fn test_error_downcast_methods() {
        use crate::notifications::error::NotificationError;
        use std::io;

        // Test downcasting NotificationError
        let notif_error = NotificationError::ChannelClosed("test_channel".to_string());
        let plugin_error: PluginError = notif_error.into();

        // Should be able to downcast back to NotificationError
        let downcast_notif = plugin_error.downcast_ref::<NotificationError>();
        assert!(downcast_notif.is_some());
        match downcast_notif.unwrap() {
            NotificationError::ChannelClosed(name) => assert_eq!(name, "test_channel"),
            _ => panic!("Unexpected notification error type"),
        }

        // Test downcasting IoError
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let plugin_error = PluginError::IoError {
            operation: "read".to_string(),
            path: "/test/path".to_string(),
            source: Some(Box::new(io_error)),
        };

        // Should be able to downcast to io::Error
        let downcast_io = plugin_error.downcast_ref::<io::Error>();
        assert!(downcast_io.is_some());
        assert_eq!(downcast_io.unwrap().kind(), io::ErrorKind::NotFound);

        // Test downcasting generic boxed error
        let boxed_error: Box<dyn std::error::Error + Send + Sync> =
            Box::new(io::Error::new(io::ErrorKind::InvalidData, "invalid data"));
        let plugin_error = PluginError::Error {
            source: boxed_error,
        };

        // Should be able to downcast to io::Error
        let downcast_generic = plugin_error.downcast_ref::<io::Error>();
        assert!(downcast_generic.is_some());
        assert_eq!(downcast_generic.unwrap().kind(), io::ErrorKind::InvalidData);

        // Test failed downcast
        let plugin_error = PluginError::Generic {
            message: "simple message".to_string(),
        };
        let failed_downcast = plugin_error.downcast_ref::<io::Error>();
        assert!(failed_downcast.is_none());
    }
}
