//! Data coordination for collecting data from multiple plugins
//!
//! The DataCoordinator manages the collection and aggregation of data exports
//! from multiple plugins, ensuring all expected plugins have provided their data
//! before triggering final output processing.

use crate::plugin::data_export::PluginDataExport;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Coordination status for tracking plugin data collection
#[derive(Clone, Debug, PartialEq)]
pub enum CoordinationStatus {
    /// Waiting for more plugins to provide data
    Pending,
    /// All expected plugins have provided data
    Complete,
    /// Coordination failed due to error or timeout
    Failed(String),
}

/// Configuration for data coordination
#[derive(Clone, Debug, PartialEq)]
pub struct CoordinationConfig {
    /// Maximum time to wait for all plugins (None for no timeout)
    pub timeout: Option<Duration>,
    /// Whether to fail fast when any plugin fails
    pub fail_fast: bool,
    /// Minimum number of plugins required for successful coordination
    pub min_plugins_required: Option<usize>,
    /// Whether to allow partial completion when timeout occurs
    pub allow_partial_completion: bool,
}

impl Default for CoordinationConfig {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(30)), // 30 second default timeout
            fail_fast: false,
            min_plugins_required: None,
            allow_partial_completion: false,
        }
    }
}

/// Data coordinator for managing plugin data collection
#[derive(Clone, Debug)]
pub struct DataCoordinator {
    /// Scan ID this coordinator is managing
    scan_id: String,
    /// Expected plugin IDs and whether they've provided data
    expected_plugins: HashMap<String, bool>,
    /// Collected plugin data exports
    collected_data: HashMap<String, Arc<PluginDataExport>>,
    /// Current coordination status
    status: CoordinationStatus,
    /// Configuration for coordination behavior
    config: CoordinationConfig,
    /// Start time for timeout tracking
    start_time: Option<Instant>,
    /// Failed plugins tracking
    failed_plugins: HashMap<String, String>, // plugin_id -> error message
}

impl DataCoordinator {
    /// Create a new data coordinator for a scan
    pub fn new(scan_id: impl Into<String>) -> Self {
        Self {
            scan_id: scan_id.into(),
            expected_plugins: HashMap::new(),
            collected_data: HashMap::new(),
            status: CoordinationStatus::Pending,
            config: CoordinationConfig::default(),
            start_time: None,
            failed_plugins: HashMap::new(),
        }
    }

    /// Create a new data coordinator with custom configuration
    pub fn with_config(scan_id: impl Into<String>, config: CoordinationConfig) -> Self {
        Self {
            scan_id: scan_id.into(),
            expected_plugins: HashMap::new(),
            collected_data: HashMap::new(),
            status: CoordinationStatus::Pending,
            config,
            start_time: None,
            failed_plugins: HashMap::new(),
        }
    }

    /// Get the coordination configuration
    pub fn config(&self) -> &CoordinationConfig {
        &self.config
    }

    /// Update the coordination configuration
    pub fn set_config(&mut self, config: CoordinationConfig) {
        self.config = config;
        self.update_status();
    }

    /// Start the coordination timer
    pub fn start(&mut self) {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
            self.update_status();
        }
    }

    /// Check if coordination has timed out
    pub fn is_timed_out(&self) -> bool {
        if let (Some(start_time), Some(timeout)) = (self.start_time, self.config.timeout) {
            start_time.elapsed() >= timeout
        } else {
            false
        }
    }

    /// Get elapsed time since coordination started
    pub fn elapsed_time(&self) -> Option<Duration> {
        self.start_time.map(|start_time| start_time.elapsed())
    }

    /// Get remaining time until timeout
    pub fn remaining_time(&self) -> Option<Duration> {
        if let (Some(start_time), Some(timeout)) = (self.start_time, self.config.timeout) {
            let elapsed = start_time.elapsed();
            if elapsed < timeout {
                Some(timeout - elapsed)
            } else {
                Some(Duration::ZERO)
            }
        } else {
            None
        }
    }

    /// Add an expected plugin that should provide data
    pub fn expect_plugin(&mut self, plugin_id: impl Into<String>) -> &mut Self {
        let plugin_id = plugin_id.into();
        self.expected_plugins.insert(plugin_id, false);
        self.update_status();
        self
    }

    /// Add multiple expected plugins
    pub fn expect_plugins(&mut self, plugin_ids: Vec<impl Into<String>>) -> &mut Self {
        for plugin_id in plugin_ids {
            self.expect_plugin(plugin_id);
        }
        self
    }

    /// Add data from a plugin
    pub fn add_data(&mut self, data: Arc<PluginDataExport>) -> Result<(), String> {
        // Validate scan ID matches
        if data.scan_id != self.scan_id {
            return Err(format!(
                "Scan ID mismatch: expected '{}', got '{}'",
                self.scan_id, data.scan_id
            ));
        }

        // Check if plugin is expected
        if !self.expected_plugins.contains_key(&data.plugin_id) {
            return Err(format!(
                "Unexpected plugin '{}' not in expected plugins list",
                data.plugin_id
            ));
        }

        // Check if plugin already provided data
        if self.collected_data.contains_key(&data.plugin_id) {
            return Err(format!("Plugin '{}' already provided data", data.plugin_id));
        }

        // Mark plugin as having provided data
        self.expected_plugins.insert(data.plugin_id.clone(), true);
        self.collected_data.insert(data.plugin_id.clone(), data);

        self.update_status();
        Ok(())
    }

    /// Check if coordination is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.status, CoordinationStatus::Complete)
    }

    /// Check if coordination has failed
    pub fn is_failed(&self) -> bool {
        matches!(self.status, CoordinationStatus::Failed(_))
    }

    /// Get the current coordination status
    pub fn status(&self) -> &CoordinationStatus {
        &self.status
    }

    /// Get the scan ID
    pub fn scan_id(&self) -> &str {
        &self.scan_id
    }

    /// Get expected plugin count
    pub fn expected_plugin_count(&self) -> usize {
        self.expected_plugins.len()
    }

    /// Get received plugin count
    pub fn received_plugin_count(&self) -> usize {
        self.collected_data.len()
    }

    /// Get pending plugins (expected but not yet received and not failed)
    pub fn pending_plugins(&self) -> Vec<String> {
        self.expected_plugins
            .keys()
            .filter(|plugin_id| {
                !self.collected_data.contains_key(*plugin_id)
                    && !self.failed_plugins.contains_key(*plugin_id)
            })
            .cloned()
            .collect()
    }

    /// Mark a plugin as failed
    pub fn mark_plugin_failed(
        &mut self,
        plugin_id: impl Into<String>,
        error: String,
    ) -> Result<(), String> {
        let plugin_id = plugin_id.into();

        if !self.expected_plugins.contains_key(&plugin_id) {
            return Err(format!(
                "Plugin '{}' not in expected plugins list",
                plugin_id
            ));
        }

        if self.failed_plugins.contains_key(&plugin_id) {
            return Err(format!("Plugin '{}' already marked as failed", plugin_id));
        }

        if self.collected_data.contains_key(&plugin_id) {
            return Err(format!("Plugin '{}' already provided data", plugin_id));
        }

        self.failed_plugins.insert(plugin_id, error);
        self.update_status();
        Ok(())
    }

    /// Get failed plugins
    pub fn failed_plugins(&self) -> &HashMap<String, String> {
        &self.failed_plugins
    }

    /// Get successful plugins (those that provided data)
    pub fn successful_plugins(&self) -> Vec<String> {
        self.collected_data.keys().cloned().collect()
    }

    /// Get all collected data
    pub fn get_all_data(&self) -> &HashMap<String, Arc<PluginDataExport>> {
        &self.collected_data
    }

    /// Get data from a specific plugin
    pub fn get_data_from(&self, plugin_id: &str) -> Option<&Arc<PluginDataExport>> {
        self.collected_data.get(plugin_id)
    }

    /// Check if we have data from a specific plugin
    pub fn has_data_from(&self, plugin_id: &str) -> bool {
        self.collected_data.contains_key(plugin_id)
    }

    /// Get coordination progress as percentage (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.expected_plugins.is_empty() {
            return 0.0;
        }

        let total_expected = self.expected_plugins.len() as f64;
        let completed = (self.collected_data.len() + self.failed_plugins.len()) as f64;
        completed / total_expected
    }

    /// Check if coordination meets minimum requirements
    pub fn meets_minimum_requirements(&self) -> bool {
        if let Some(min_required) = self.config.min_plugins_required {
            self.collected_data.len() >= min_required
        } else {
            true
        }
    }

    /// Force completion with current data (useful for timeout scenarios)
    pub fn force_completion(&mut self) -> Result<(), String> {
        if !self.meets_minimum_requirements() {
            return Err(format!(
                "Minimum requirements not met: {} < {}",
                self.collected_data.len(),
                self.config.min_plugins_required.unwrap_or(0)
            ));
        }

        self.status = CoordinationStatus::Complete;
        Ok(())
    }

    /// Force failure with reason
    pub fn force_failure(&mut self, reason: String) {
        self.status = CoordinationStatus::Failed(reason);
    }

    /// Clear all data and reset coordinator
    pub fn clear(&mut self) {
        self.expected_plugins.clear();
        self.collected_data.clear();
        self.failed_plugins.clear();
        self.status = CoordinationStatus::Pending;
        self.start_time = None;
    }

    /// Reset for new scan while keeping expected plugins and config
    pub fn reset_for_new_scan(&mut self, new_scan_id: impl Into<String>) {
        self.scan_id = new_scan_id.into();
        self.collected_data.clear();
        self.failed_plugins.clear();

        // Reset all expected plugins to not received
        for received in self.expected_plugins.values_mut() {
            *received = false;
        }

        self.status = CoordinationStatus::Pending;
        self.start_time = None;
    }

    /// Update coordination status based on current state
    fn update_status(&mut self) {
        // Don't change status if already failed
        if matches!(self.status, CoordinationStatus::Failed(_)) {
            return;
        }

        if self.expected_plugins.is_empty() {
            self.status = CoordinationStatus::Pending;
            return;
        }

        // Check for timeout
        if self.is_timed_out() {
            if self.config.allow_partial_completion && self.meets_minimum_requirements() {
                self.status = CoordinationStatus::Complete;
            } else {
                self.status = CoordinationStatus::Failed(format!(
                    "Timeout: received {}/{} plugins within {:?}",
                    self.collected_data.len(),
                    self.expected_plugins.len(),
                    self.config.timeout.unwrap()
                ));
            }
            return;
        }

        // Check for fail fast condition
        if self.config.fail_fast && !self.failed_plugins.is_empty() {
            let failed_list: Vec<_> = self.failed_plugins.keys().collect();
            self.status =
                CoordinationStatus::Failed(format!("Fail fast: plugins {:?} failed", failed_list));
            return;
        }

        // Check if all expected plugins are accounted for (success or failure)
        let total_accounted = self.collected_data.len() + self.failed_plugins.len();
        if total_accounted == self.expected_plugins.len() {
            if self.meets_minimum_requirements() {
                self.status = CoordinationStatus::Complete;
            } else {
                self.status = CoordinationStatus::Failed(format!(
                    "Minimum requirements not met: {} successful < {} required",
                    self.collected_data.len(),
                    self.config.min_plugins_required.unwrap_or(0)
                ));
            }
        } else {
            self.status = CoordinationStatus::Pending;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::data_export::{DataPayload, ExportFormat, ExportHints};
    use std::collections::HashMap as StdHashMap;

    #[test]
    fn test_data_coordinator_new() {
        let coordinator = DataCoordinator::new("scan_123");

        assert_eq!(coordinator.scan_id(), "scan_123");
        assert_eq!(coordinator.expected_plugin_count(), 0);
        assert_eq!(coordinator.received_plugin_count(), 0);
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);
        assert!(!coordinator.is_complete());
        assert!(!coordinator.is_failed());
    }

    #[test]
    fn test_data_coordinator_expect_plugin() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugin("plugin1");

        assert_eq!(coordinator.expected_plugin_count(), 1);
        assert_eq!(coordinator.received_plugin_count(), 0);
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);
        assert!(!coordinator.is_complete());

        let pending = coordinator.pending_plugins();
        assert_eq!(pending.len(), 1);
        assert!(pending.contains(&"plugin1".to_string()));
    }

    #[test]
    fn test_data_coordinator_expect_multiple_plugins() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugins(vec!["plugin1", "plugin2", "plugin3"]);

        assert_eq!(coordinator.expected_plugin_count(), 3);
        assert_eq!(coordinator.received_plugin_count(), 0);
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);

        let pending = coordinator.pending_plugins();
        assert_eq!(pending.len(), 3);
        assert!(pending.contains(&"plugin1".to_string()));
        assert!(pending.contains(&"plugin2".to_string()));
        assert!(pending.contains(&"plugin3".to_string()));
    }

    #[test]
    fn test_data_coordinator_expect_plugins_fluent_interface() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator
            .expect_plugin("plugin1")
            .expect_plugin("plugin2")
            .expect_plugin("plugin3");

        assert_eq!(coordinator.expected_plugin_count(), 3);
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);
    }

    #[test]
    fn test_data_coordinator_add_data_success() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugin("plugin1");

        let payload = DataPayload::raw("test data".to_string(), None);
        let data = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload));

        let result = coordinator.add_data(data);
        assert!(result.is_ok());
        assert_eq!(coordinator.received_plugin_count(), 1);
        assert_eq!(coordinator.status(), &CoordinationStatus::Complete);
        assert!(coordinator.is_complete());
    }

    #[test]
    fn test_data_coordinator_add_data_scan_id_mismatch() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugin("plugin1");

        let payload = DataPayload::raw("test data".to_string(), None);
        let data = Arc::new(PluginDataExport::new("plugin1", "wrong_scan", payload));

        let result = coordinator.add_data(data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Scan ID mismatch"));
        assert_eq!(coordinator.received_plugin_count(), 0);
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);
    }

    #[test]
    fn test_data_coordinator_add_data_unexpected_plugin() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugin("plugin1");

        let payload = DataPayload::raw("test data".to_string(), None);
        let data = Arc::new(PluginDataExport::new("plugin2", "scan_123", payload));

        let result = coordinator.add_data(data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unexpected plugin"));
        assert_eq!(coordinator.received_plugin_count(), 0);
    }

    #[test]
    fn test_data_coordinator_add_data_duplicate_plugin() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugin("plugin1");

        let payload1 = DataPayload::raw("test data 1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));

        let payload2 = DataPayload::raw("test data 2".to_string(), None);
        let data2 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload2));

        let result1 = coordinator.add_data(data1);
        assert!(result1.is_ok());

        let result2 = coordinator.add_data(data2);
        assert!(result2.is_err());
        assert!(result2.unwrap_err().contains("already provided data"));
        assert_eq!(coordinator.received_plugin_count(), 1);
    }

    #[test]
    fn test_data_coordinator_partial_completion() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugins(vec!["plugin1", "plugin2", "plugin3"]);

        // Add data from first plugin
        let payload1 = DataPayload::raw("data 1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));
        coordinator.add_data(data1).unwrap();

        assert_eq!(coordinator.received_plugin_count(), 1);
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);
        assert!(!coordinator.is_complete());

        let pending = coordinator.pending_plugins();
        assert_eq!(pending.len(), 2);
        assert!(!pending.contains(&"plugin1".to_string()));
        assert!(pending.contains(&"plugin2".to_string()));
        assert!(pending.contains(&"plugin3".to_string()));
    }

    #[test]
    fn test_data_coordinator_full_completion() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugins(vec!["plugin1", "plugin2"]);

        // Add data from both plugins
        let payload1 = DataPayload::raw("data 1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));
        coordinator.add_data(data1).unwrap();

        let payload2 = DataPayload::key_value(StdHashMap::new());
        let data2 = Arc::new(PluginDataExport::new("plugin2", "scan_123", payload2));
        coordinator.add_data(data2).unwrap();

        assert_eq!(coordinator.received_plugin_count(), 2);
        assert_eq!(coordinator.status(), &CoordinationStatus::Complete);
        assert!(coordinator.is_complete());
        assert!(coordinator.pending_plugins().is_empty());
    }

    #[test]
    fn test_data_coordinator_clear() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugins(vec!["plugin1", "plugin2"]);

        let payload = DataPayload::raw("data".to_string(), None);
        let data = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload));
        coordinator.add_data(data).unwrap();

        assert_eq!(coordinator.expected_plugin_count(), 2);
        assert_eq!(coordinator.received_plugin_count(), 1);

        coordinator.clear();

        assert_eq!(coordinator.expected_plugin_count(), 0);
        assert_eq!(coordinator.received_plugin_count(), 0);
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);
    }

    #[test]
    fn test_data_coordinator_reset_for_new_scan() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugins(vec!["plugin1", "plugin2"]);

        let payload = DataPayload::raw("data".to_string(), None);
        let data = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload));
        coordinator.add_data(data).unwrap();

        assert_eq!(coordinator.scan_id(), "scan_123");
        assert_eq!(coordinator.received_plugin_count(), 1);

        coordinator.reset_for_new_scan("scan_456");

        assert_eq!(coordinator.scan_id(), "scan_456");
        assert_eq!(coordinator.expected_plugin_count(), 2); // Plugins still expected
        assert_eq!(coordinator.received_plugin_count(), 0); // Data cleared
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);
        assert_eq!(coordinator.pending_plugins().len(), 2);
    }

    #[test]
    fn test_data_coordinator_status_transitions() {
        let mut coordinator = DataCoordinator::new("scan_123");

        // Initially pending with no expected plugins
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);

        // Add expected plugins - still pending
        coordinator.expect_plugins(vec!["plugin1", "plugin2"]);
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);

        // Add first plugin data - still pending
        let payload1 = DataPayload::raw("data 1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));
        coordinator.add_data(data1).unwrap();
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);

        // Add second plugin data - now complete
        let payload2 = DataPayload::raw("data 2".to_string(), None);
        let data2 = Arc::new(PluginDataExport::new("plugin2", "scan_123", payload2));
        coordinator.add_data(data2).unwrap();
        assert_eq!(coordinator.status(), &CoordinationStatus::Complete);
    }

    #[test]
    fn test_data_coordinator_clone() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugin("plugin1");

        let cloned = coordinator.clone();

        assert_eq!(coordinator.scan_id(), cloned.scan_id());
        assert_eq!(
            coordinator.expected_plugin_count(),
            cloned.expected_plugin_count()
        );
        assert_eq!(coordinator.status(), cloned.status());
    }

    // CoordinationConfig tests
    #[test]
    fn test_coordination_config_default() {
        let config = CoordinationConfig::default();

        assert_eq!(config.timeout, Some(Duration::from_secs(30)));
        assert_eq!(config.fail_fast, false);
        assert_eq!(config.min_plugins_required, None);
        assert_eq!(config.allow_partial_completion, false);
    }

    #[test]
    fn test_coordination_config_equality() {
        let config1 = CoordinationConfig::default();
        let config2 = CoordinationConfig::default();
        let mut config3 = CoordinationConfig::default();
        config3.fail_fast = true;

        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_coordination_config_clone() {
        let original = CoordinationConfig {
            timeout: Some(Duration::from_secs(60)),
            fail_fast: true,
            min_plugins_required: Some(2),
            allow_partial_completion: true,
        };
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    // Advanced DataCoordinator tests
    #[test]
    fn test_data_coordinator_with_config() {
        let config = CoordinationConfig {
            timeout: Some(Duration::from_secs(60)),
            fail_fast: true,
            min_plugins_required: Some(2),
            allow_partial_completion: false,
        };

        let coordinator = DataCoordinator::with_config("scan_123", config.clone());

        assert_eq!(coordinator.scan_id(), "scan_123");
        assert_eq!(coordinator.config(), &config);
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);
    }

    #[test]
    fn test_data_coordinator_set_config() {
        let mut coordinator = DataCoordinator::new("scan_123");
        let original_config = coordinator.config().clone();

        let new_config = CoordinationConfig {
            timeout: Some(Duration::from_secs(120)),
            fail_fast: true,
            min_plugins_required: Some(3),
            allow_partial_completion: true,
        };

        coordinator.set_config(new_config.clone());

        assert_ne!(coordinator.config(), &original_config);
        assert_eq!(coordinator.config(), &new_config);
    }

    #[test]
    fn test_data_coordinator_start_timer() {
        let mut coordinator = DataCoordinator::new("scan_123");

        assert_eq!(coordinator.elapsed_time(), None);
        assert_eq!(coordinator.remaining_time(), None);
        assert!(!coordinator.is_timed_out());

        coordinator.start();

        assert!(coordinator.elapsed_time().is_some());
        assert!(coordinator.remaining_time().is_some());
        assert!(coordinator.elapsed_time().unwrap() < Duration::from_millis(100));
        // Should be very small
    }

    #[test]
    fn test_data_coordinator_mark_plugin_failed() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugins(vec!["plugin1", "plugin2"]);

        let result = coordinator.mark_plugin_failed("plugin1", "Test error".to_string());
        assert!(result.is_ok());

        assert_eq!(coordinator.failed_plugins().len(), 1);
        assert_eq!(
            coordinator.failed_plugins().get("plugin1"),
            Some(&"Test error".to_string())
        );
        assert_eq!(coordinator.progress(), 0.5); // 1 out of 2 plugins accounted for

        let pending = coordinator.pending_plugins();
        assert_eq!(pending.len(), 1);
        assert!(pending.contains(&"plugin2".to_string()));
    }

    #[test]
    fn test_data_coordinator_mark_plugin_failed_errors() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugin("plugin1");

        // Try to mark non-existent plugin as failed
        let result = coordinator.mark_plugin_failed("plugin2", "Error".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in expected plugins list"));

        // Mark plugin as failed
        coordinator
            .mark_plugin_failed("plugin1", "Error".to_string())
            .unwrap();

        // Try to mark same plugin as failed again
        let result = coordinator.mark_plugin_failed("plugin1", "Another error".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already marked as failed"));

        // Reset and add data, then try to mark as failed
        coordinator.clear();
        coordinator.expect_plugin("plugin1");

        let payload = DataPayload::raw("data".to_string(), None);
        let data = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload));
        coordinator.add_data(data).unwrap();

        let result = coordinator.mark_plugin_failed("plugin1", "Error".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already provided data"));
    }

    #[test]
    fn test_data_coordinator_fail_fast() {
        let config = CoordinationConfig {
            fail_fast: true,
            ..Default::default()
        };
        let mut coordinator = DataCoordinator::with_config("scan_123", config);
        coordinator.expect_plugins(vec!["plugin1", "plugin2", "plugin3"]);

        // Add successful data from one plugin
        let payload1 = DataPayload::raw("data1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));
        coordinator.add_data(data1).unwrap();
        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);

        // Mark another plugin as failed - should trigger fail fast
        coordinator
            .mark_plugin_failed("plugin2", "Test failure".to_string())
            .unwrap();

        assert!(coordinator.is_failed());
        if let CoordinationStatus::Failed(msg) = coordinator.status() {
            assert!(msg.contains("Fail fast"));
            assert!(msg.contains("plugin2"));
        }
    }

    #[test]
    fn test_data_coordinator_minimum_requirements() {
        let config = CoordinationConfig {
            min_plugins_required: Some(2),
            ..Default::default()
        };
        let mut coordinator = DataCoordinator::with_config("scan_123", config);
        coordinator.expect_plugins(vec!["plugin1", "plugin2", "plugin3"]);

        // Add data from one plugin
        let payload1 = DataPayload::raw("data1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));
        coordinator.add_data(data1).unwrap();

        // Mark other plugins as failed
        coordinator
            .mark_plugin_failed("plugin2", "Error".to_string())
            .unwrap();
        coordinator
            .mark_plugin_failed("plugin3", "Error".to_string())
            .unwrap();

        // Should fail due to minimum requirements not met
        assert!(coordinator.is_failed());
        if let CoordinationStatus::Failed(msg) = coordinator.status() {
            assert!(msg.contains("Minimum requirements not met"));
            assert!(msg.contains("1 successful < 2 required"));
        }
    }

    #[test]
    fn test_data_coordinator_minimum_requirements_met() {
        let config = CoordinationConfig {
            min_plugins_required: Some(2),
            ..Default::default()
        };
        let mut coordinator = DataCoordinator::with_config("scan_123", config);
        coordinator.expect_plugins(vec!["plugin1", "plugin2", "plugin3"]);

        // Add data from two plugins
        let payload1 = DataPayload::raw("data1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));
        coordinator.add_data(data1).unwrap();

        let payload2 = DataPayload::raw("data2".to_string(), None);
        let data2 = Arc::new(PluginDataExport::new("plugin2", "scan_123", payload2));
        coordinator.add_data(data2).unwrap();

        // Mark third plugin as failed
        coordinator
            .mark_plugin_failed("plugin3", "Error".to_string())
            .unwrap();

        // Should complete successfully (minimum requirements met)
        assert!(coordinator.is_complete());
    }

    #[test]
    fn test_data_coordinator_progress_calculation() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugins(vec!["plugin1", "plugin2", "plugin3", "plugin4"]);

        // Initially no progress
        assert_eq!(coordinator.progress(), 0.0);

        // Add data from one plugin - 25% progress
        let payload1 = DataPayload::raw("data1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));
        coordinator.add_data(data1).unwrap();
        assert_eq!(coordinator.progress(), 0.25);

        // Mark another as failed - 50% progress
        coordinator
            .mark_plugin_failed("plugin2", "Error".to_string())
            .unwrap();
        assert_eq!(coordinator.progress(), 0.5);

        // Add two more - 100% progress
        let payload3 = DataPayload::raw("data3".to_string(), None);
        let data3 = Arc::new(PluginDataExport::new("plugin3", "scan_123", payload3));
        coordinator.add_data(data3).unwrap();

        let payload4 = DataPayload::raw("data4".to_string(), None);
        let data4 = Arc::new(PluginDataExport::new("plugin4", "scan_123", payload4));
        coordinator.add_data(data4).unwrap();

        assert_eq!(coordinator.progress(), 1.0);
    }

    #[test]
    fn test_data_coordinator_data_retrieval() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugins(vec!["plugin1", "plugin2"]);

        // Add data from plugins
        let payload1 = DataPayload::raw("data1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));
        let data1_clone = data1.clone();
        coordinator.add_data(data1).unwrap();

        let payload2 = DataPayload::key_value(StdHashMap::new());
        let data2 = Arc::new(PluginDataExport::new("plugin2", "scan_123", payload2));
        let data2_clone = data2.clone();
        coordinator.add_data(data2).unwrap();

        // Test data retrieval
        assert!(coordinator.has_data_from("plugin1"));
        assert!(coordinator.has_data_from("plugin2"));
        assert!(!coordinator.has_data_from("plugin3"));

        assert_eq!(coordinator.get_data_from("plugin1"), Some(&data1_clone));
        assert_eq!(coordinator.get_data_from("plugin2"), Some(&data2_clone));
        assert_eq!(coordinator.get_data_from("plugin3"), None);

        let all_data = coordinator.get_all_data();
        assert_eq!(all_data.len(), 2);
        assert!(all_data.contains_key("plugin1"));
        assert!(all_data.contains_key("plugin2"));

        let successful = coordinator.successful_plugins();
        assert_eq!(successful.len(), 2);
        assert!(successful.contains(&"plugin1".to_string()));
        assert!(successful.contains(&"plugin2".to_string()));
    }

    #[test]
    fn test_data_coordinator_force_completion() {
        let config = CoordinationConfig {
            min_plugins_required: Some(2),
            ..Default::default()
        };
        let mut coordinator = DataCoordinator::with_config("scan_123", config);
        coordinator.expect_plugins(vec!["plugin1", "plugin2", "plugin3"]);

        // Add data from only one plugin
        let payload1 = DataPayload::raw("data1".to_string(), None);
        let data1 = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload1));
        coordinator.add_data(data1).unwrap();

        // Try to force completion - should fail due to minimum requirements
        let result = coordinator.force_completion();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Minimum requirements not met"));

        // Add more data to meet requirements
        let payload2 = DataPayload::raw("data2".to_string(), None);
        let data2 = Arc::new(PluginDataExport::new("plugin2", "scan_123", payload2));
        coordinator.add_data(data2).unwrap();

        // Now force completion should work
        let result = coordinator.force_completion();
        assert!(result.is_ok());
        assert!(coordinator.is_complete());
    }

    #[test]
    fn test_data_coordinator_force_failure() {
        let mut coordinator = DataCoordinator::new("scan_123");
        coordinator.expect_plugin("plugin1");

        assert_eq!(coordinator.status(), &CoordinationStatus::Pending);

        coordinator.force_failure("Test failure reason".to_string());

        assert!(coordinator.is_failed());
        if let CoordinationStatus::Failed(msg) = coordinator.status() {
            assert_eq!(msg, "Test failure reason");
        }
    }

    #[test]
    fn test_data_coordinator_reset_preserves_config() {
        let config = CoordinationConfig {
            timeout: Some(Duration::from_secs(120)),
            fail_fast: true,
            min_plugins_required: Some(3),
            allow_partial_completion: true,
        };

        let mut coordinator = DataCoordinator::with_config("scan_123", config.clone());
        coordinator.expect_plugins(vec!["plugin1", "plugin2"]);
        coordinator.start();

        let payload = DataPayload::raw("data".to_string(), None);
        let data = Arc::new(PluginDataExport::new("plugin1", "scan_123", payload));
        coordinator.add_data(data).unwrap();

        coordinator.reset_for_new_scan("scan_456");

        assert_eq!(coordinator.scan_id(), "scan_456");
        assert_eq!(coordinator.config(), &config); // Config preserved
        assert_eq!(coordinator.expected_plugin_count(), 2); // Expected plugins preserved
        assert_eq!(coordinator.received_plugin_count(), 0); // Data cleared
        assert_eq!(coordinator.failed_plugins().len(), 0); // Failed plugins cleared
        assert_eq!(coordinator.elapsed_time(), None); // Timer reset
    }
}
