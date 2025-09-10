//! Plugin activation helper module
//!
//! This module handles the logic for matching command segments to plugins
//! and managing plugin activation based on user commands and auto-activation rules.

use crate::app::cli::command_segmenter::CommandSegment;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::types::{PluginInfo, PluginType};
use std::collections::HashSet;

/// Helper struct for managing plugin activation logic
pub struct PluginActivator {
    /// Plugins marked for auto-activation
    auto_active_plugins: HashSet<String>,
}

impl PluginActivator {
    /// Create a new PluginActivator
    pub fn new(auto_active_plugins: Vec<String>) -> Self {
        Self {
            auto_active_plugins: auto_active_plugins.into_iter().collect(),
        }
    }

    /// Check if a command segment matches a plugin
    pub fn matches_segment(
        &self,
        plugin_name: &str,
        segment: &CommandSegment,
        plugin_functions: &[crate::plugin::types::PluginFunction],
    ) -> bool {
        // Check if plugin name matches
        if plugin_name == segment.command_name {
            return true;
        }

        // Check if any function name or alias matches
        for function in plugin_functions {
            if function.name == segment.command_name
                || function.aliases.contains(&segment.command_name)
            {
                return true;
            }
        }

        false
    }

    /// Process command segments to find plugins to activate
    pub fn process_segments(
        &self,
        segments: &[CommandSegment],
        all_plugins: &[(
            String,
            Vec<crate::plugin::types::PluginFunction>,
            Option<PluginInfo>,
        )],
    ) -> PluginResult<ActivationResult> {
        let mut segments_to_process: Vec<CommandSegment> = segments.to_vec();
        let mut plugins_to_activate: Vec<(String, Vec<String>)> = Vec::new();
        let mut active_output_plugin: Option<String> = None;

        // Process each plugin to check for segment matches
        for (plugin_name, functions, plugin_info) in all_plugins {
            let mut segments_matched = Vec::new();

            // Check if any command segments match this plugin
            for (index, segment) in segments_to_process.iter().enumerate() {
                if self.matches_segment(plugin_name, segment, functions) {
                    // Build args from segment
                    plugins_to_activate.push((plugin_name.clone(), segment.args.clone()));

                    // Check if it's an Output plugin - segment match always wins
                    if let Some(info) = plugin_info {
                        if info.plugin_type == PluginType::Output {
                            active_output_plugin = Some(plugin_name.clone());
                        }
                    }

                    segments_matched.push(index);
                }
            }

            // Remove matched segments (in reverse order to preserve indices)
            for &index in segments_matched.iter().rev() {
                segments_to_process.remove(index);
            }
        }

        // Check for unknown commands
        if !segments_to_process.is_empty() {
            return Err(PluginError::PluginNotFound {
                plugin_name: segments_to_process[0].command_name.clone(),
            });
        }

        Ok(ActivationResult {
            plugins_to_activate,
            active_output_plugin,
        })
    }

    /// Process auto-activation for plugins
    pub fn process_auto_activation(
        &self,
        all_plugins: &[(String, Option<PluginInfo>)],
        active_output_plugin: &mut Option<String>,
    ) -> Vec<(String, Vec<String>)> {
        let mut auto_activated = Vec::new();

        for (plugin_name, plugin_info) in all_plugins {
            if self.auto_active_plugins.contains(plugin_name) {
                // Auto-activate with empty args
                auto_activated.push((plugin_name.clone(), Vec::new()));

                // Check if it's an Output plugin - only set if no Output plugin chosen yet
                if active_output_plugin.is_none() {
                    if let Some(info) = plugin_info {
                        if info.plugin_type == PluginType::Output {
                            *active_output_plugin = Some(plugin_name.clone());
                        }
                    }
                }
            }
        }

        auto_activated
    }

    /// Apply Output plugin uniqueness constraint
    pub fn apply_output_constraint(
        &self,
        plugins_to_activate: Vec<(String, Vec<String>)>,
        active_output_plugin: &Option<String>,
        all_plugins: &[(String, Option<PluginInfo>)],
    ) -> Vec<(String, Vec<String>)> {
        if let Some(ref chosen_output) = active_output_plugin {
            let mut filtered = Vec::new();

            for (plugin_name, args) in plugins_to_activate {
                // Check if this is an Output plugin
                let is_output = all_plugins
                    .iter()
                    .find(|(name, _)| name == &plugin_name)
                    .and_then(|(_, info)| info.as_ref())
                    .map(|info| info.plugin_type == PluginType::Output)
                    .unwrap_or(false);

                // Keep non-Output plugins or the chosen Output plugin
                if !is_output || &plugin_name == chosen_output {
                    filtered.push((plugin_name, args));
                } else {
                    log::debug!(
                        "Skipping Output plugin '{}' due to uniqueness constraint (using '{}')",
                        plugin_name,
                        chosen_output
                    );
                }
            }

            filtered
        } else {
            plugins_to_activate
        }
    }
}

/// Result of plugin activation processing
pub struct ActivationResult {
    /// Plugins to activate with their arguments
    pub plugins_to_activate: Vec<(String, Vec<String>)>,
    /// The chosen Output plugin (if any)
    pub active_output_plugin: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::types::PluginFunction;

    #[test]
    fn test_matches_segment_by_name() {
        let activator = PluginActivator::new(vec![]);
        let segment = CommandSegment {
            command_name: "test".to_string(),
            args: vec![],
        };
        let functions = vec![];

        assert!(activator.matches_segment("test", &segment, &functions));
        assert!(!activator.matches_segment("other", &segment, &functions));
    }

    #[test]
    fn test_matches_segment_by_function() {
        let activator = PluginActivator::new(vec![]);
        let segment = CommandSegment {
            command_name: "run".to_string(),
            args: vec![],
        };
        let functions = vec![PluginFunction {
            name: "execute".to_string(),
            description: "Execute plugin".to_string(),
            aliases: vec!["run".to_string(), "start".to_string()],
        }];

        assert!(activator.matches_segment("test", &segment, &functions));
    }

    #[test]
    fn test_auto_activation() {
        let activator = PluginActivator::new(vec!["auto1".to_string(), "auto2".to_string()]);
        let mut active_output = None;

        let plugins = vec![
            ("auto1".to_string(), None),
            ("manual".to_string(), None),
            ("auto2".to_string(), None),
        ];

        let activated = activator.process_auto_activation(&plugins, &mut active_output);

        assert_eq!(activated.len(), 2);
        assert!(activated.iter().any(|(name, _)| name == "auto1"));
        assert!(activated.iter().any(|(name, _)| name == "auto2"));
        assert!(!activated.iter().any(|(name, _)| name == "manual"));
    }

    #[test]
    fn test_output_constraint() {
        let activator = PluginActivator::new(vec![]);

        let plugins_to_activate = vec![
            ("plugin1".to_string(), vec![]),
            ("output1".to_string(), vec![]),
            ("plugin2".to_string(), vec![]),
            ("output2".to_string(), vec![]),
        ];

        let all_plugins = vec![
            (
                "plugin1".to_string(),
                Some(PluginInfo {
                    name: "plugin1".to_string(),
                    version: "1.0.0".to_string(),
                    description: "".to_string(),
                    author: "".to_string(),
                    api_version: 1,
                    plugin_type: PluginType::Processing,
                    functions: vec![],
                    required: crate::scanner::types::ScanRequires::NONE,
                    auto_active: false,
                }),
            ),
            (
                "output1".to_string(),
                Some(PluginInfo {
                    name: "output1".to_string(),
                    version: "1.0.0".to_string(),
                    description: "".to_string(),
                    author: "".to_string(),
                    api_version: 1,
                    plugin_type: PluginType::Output,
                    functions: vec![],
                    required: crate::scanner::types::ScanRequires::NONE,
                    auto_active: false,
                }),
            ),
            (
                "plugin2".to_string(),
                Some(PluginInfo {
                    name: "plugin2".to_string(),
                    version: "1.0.0".to_string(),
                    description: "".to_string(),
                    author: "".to_string(),
                    api_version: 1,
                    plugin_type: PluginType::Processing,
                    functions: vec![],
                    required: crate::scanner::types::ScanRequires::NONE,
                    auto_active: false,
                }),
            ),
            (
                "output2".to_string(),
                Some(PluginInfo {
                    name: "output2".to_string(),
                    version: "1.0.0".to_string(),
                    description: "".to_string(),
                    author: "".to_string(),
                    api_version: 1,
                    plugin_type: PluginType::Output,
                    functions: vec![],
                    required: crate::scanner::types::ScanRequires::NONE,
                    auto_active: false,
                }),
            ),
        ];

        let active_output = Some("output1".to_string());
        let filtered =
            activator.apply_output_constraint(plugins_to_activate, &active_output, &all_plugins);

        assert_eq!(filtered.len(), 3); // plugin1, output1, plugin2 (not output2)
        assert!(filtered.iter().any(|(name, _)| name == "plugin1"));
        assert!(filtered.iter().any(|(name, _)| name == "output1"));
        assert!(filtered.iter().any(|(name, _)| name == "plugin2"));
        assert!(!filtered.iter().any(|(name, _)| name == "output2"));
    }
}
