//! Tests for plugin error handling

use crate::plugin::error::*;

#[test]
fn test_version_incompatible_error() {
    let error = PluginError::VersionIncompatible {
        message: "Plugin requires API version 2025.0.0, but system has 2024.0.0".to_string(),
    };

    assert_eq!(
        error.to_string(),
        "Version incompatible: Plugin requires API version 2025.0.0, but system has 2024.0.0"
    );
}

#[test]
fn test_load_error() {
    let error = PluginError::LoadError {
        plugin_name: "dump".to_string(),
        cause: "Missing required dependencies".to_string(),
    };

    assert_eq!(
        error.to_string(),
        "Failed to load plugin 'dump': Missing required dependencies"
    );
}

#[test]
fn test_execution_error() {
    let error = PluginError::ExecutionError {
        plugin_name: "dump".to_string(),
        operation: "process_message".to_string(),
        cause: "Invalid message format".to_string(),
    };

    assert_eq!(
        error.to_string(),
        "Plugin 'dump' failed during 'process_message': Invalid message format"
    );
}

#[test]
fn test_async_error() {
    let error = PluginError::AsyncError {
        message: "Tokio runtime unavailable".to_string(),
    };

    assert_eq!(
        error.to_string(),
        "Async operation error: Tokio runtime unavailable"
    );
}

#[test]
fn test_generic_error() {
    let error = PluginError::Generic {
        message: "Unknown plugin system failure".to_string(),
    };

    assert_eq!(error.to_string(), "Unknown plugin system failure");
}

#[test]
fn test_plugin_result_type_alias() {
    let success: PluginResult<String> = Ok("success".to_string());
    let failure: PluginResult<String> = Err(PluginError::Generic {
        message: "test failure".to_string(),
    });

    assert!(success.is_ok());
    assert!(failure.is_err());

    match failure {
        Err(PluginError::Generic { message }) => {
            assert_eq!(message, "test failure");
        }
        _ => panic!("Expected Generic error"),
    }
}

#[test]
fn test_error_debug_formatting() {
    let error = PluginError::ExecutionError {
        plugin_name: "test".to_string(),
        operation: "init".to_string(),
        cause: "failed".to_string(),
    };

    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("ExecutionError"));
    assert!(debug_str.contains("test"));
    assert!(debug_str.contains("init"));
    assert!(debug_str.contains("failed"));
}
