//! Plugin activation helper module
//!
//! This module handles the logic for matching command segments to plugins
//! and managing plugin activation based on user commands and auto-activation rules.

use crate::app::cli::segmenter::CommandSegment;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::types::{PluginInfo, PluginType};
use std::collections::{HashMap, HashSet};

/// Helper struct for managing plugin activation logic
pub struct PluginActivator {
    available_plugins: HashMap<String, PluginInfo>,
}

impl PluginActivator {
    /// Create a new PluginActivator
    pub fn new(available_plugins: HashMap<String, PluginInfo>) -> Self {
        Self {
            available_plugins: available_plugins.clone(),
        }
    }

    /// Check if a command segment matches a plugin
    pub fn match_segment(&self, plugin_name: &str, segment: &CommandSegment) -> Option<String> {
        // Check if the plugin name matches
        if plugin_name == segment.command_name {
            return Some(plugin_name.to_string());
        }
        // Check if any plugin functions matches
        if let Some(plugin_info) = self.available_plugins.get(plugin_name) {
            for function_name in plugin_info.functions.iter() {
                if function_name == &segment.command_name {
                    return Some(function_name.to_string());
                }
            }
        }
        None
    }

    /// Process command segments to find plugins to activate
    pub fn process_segments(
        &mut self,
        segments: &[CommandSegment],
    ) -> PluginResult<HashMap<String, Vec<String>>> {
        let segments_to_process: Vec<CommandSegment> = segments.to_vec();
        let mut plugins_to_activate: HashMap<String, Vec<String>> = HashMap::new();
        let mut matched_segment_indices: HashSet<usize> = HashSet::new();
        let mut active_output_plugin: Option<String> = None;
        let mut active_output_args: Option<Vec<String>> = None;

        // Process each plugin to check for segment matches
        for (plugin_name, plugin_info) in self.available_plugins.iter() {
            // Check if any command segments match this plugin
            for (index, segment) in segments_to_process.iter().enumerate() {
                if let Some(matched_name) = self.match_segment(plugin_name, segment) {
                    // activate it or track the last output plugin
                    if plugin_info.plugin_type == PluginType::Output {
                        // last Output wins; preserve args from the matching segment
                        active_output_plugin = Some(matched_name.to_string());
                        active_output_args = Some(segment.args.clone());
                    } else {
                        // preserve args from the matched segment
                        plugins_to_activate.insert(matched_name.to_string(), segment.args.clone());
                    }
                    matched_segment_indices.insert(index);
                    break;
                }
            }
            // or if it is auto-activated
            if plugin_info.auto_active {
                if !plugins_to_activate.contains_key(plugin_name) {
                    if plugin_info.plugin_type == PluginType::Output {
                        // last Output wins; if chosen via auto-activation, use a sensible default arg list
                        let plugin_name = plugin_name.to_string();
                        active_output_plugin = Some(plugin_name.clone());
                        active_output_args = Some(vec![plugin_name]);
                    } else {
                        // only insert if not already matched; don't overwrite matched args
                        plugins_to_activate.insert(plugin_name.to_string(), vec![]);
                    }
                }
            }
        }

        // Only activate the last active Output plugin, preserving its args if we have them
        if let Some(output_plugin) = &active_output_plugin {
            let plugin_name = output_plugin.to_string();
            if !plugins_to_activate.contains_key(&plugin_name) {
                let plugin_args = active_output_args.unwrap_or_else(|| vec![plugin_name.clone()]);
                plugins_to_activate.insert(plugin_name, plugin_args);
            }
        }

        let mut unmatched_iter = segments_to_process
            .iter()
            .enumerate()
            .filter(|(index, _)| !matched_segment_indices.contains(index))
            .map(|(_, seg)| seg);

        // Check for unknown commands
        if let Some(unmatched) = unmatched_iter.next() {
            return Err(PluginError::PluginNotFound {
                plugin_name: unmatched.command_name.clone(),
            });
        }

        Ok(plugins_to_activate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_segment_by_name() {
        let activator = PluginActivator::new(HashMap::new());
        let segment = CommandSegment {
            command_name: "test".to_string(),
            args: vec!["test".to_string()],
        };
        assert!(activator.match_segment("test", &segment).is_some());
        assert!(activator.match_segment("other", &segment).is_none());
    }

    #[test]
    fn test_matches_segment_by_function() {
        let activator = PluginActivator::new(HashMap::new());
        let segment = CommandSegment {
            command_name: "run".to_string(),
            args: vec!["run".to_string()],
        };
        assert!(activator.match_segment("run", &segment).is_some());
        assert!(activator.match_segment("test", &segment).is_none());
    }
}
