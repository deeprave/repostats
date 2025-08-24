//! Command Line Segmentation
//!
//! This module handles the segmentation of command line arguments by command boundaries.
//! It splits the command line into global arguments and command-specific argument segments.
//! This is a pure, self-contained argument parser with no external dependencies or concerns.

use anyhow::Result;

/// Represents a segment of command line arguments for a specific command
#[derive(Debug, Clone, PartialEq)]
pub struct CommandSegment {
    /// Command name
    pub command_name: String,
    /// Arguments for this command
    pub args: Vec<String>,
}

/// Command line segmenter that splits arguments by command boundaries
#[derive(Debug)]
pub struct CommandSegmenter {
    known_commands: Vec<String>,
}

impl CommandSegmenter {
    /// Create a new command segmenter with a list of known commands
    pub fn with_commands(commands: Vec<String>) -> Self {
        Self {
            known_commands: commands,
        }
    }

    /// Segment remaining arguments after global args have been identified
    ///
    /// This method takes the original args and the pre-collected global args,
    /// then processes the remaining arguments for command segmentation.
    ///
    /// Example:
    /// - args: ["repostats", "--verbose", "--config", "file.toml", "scan", "--since", "1week", "status", "--format", "json"]
    /// - global_args: ["repostats", "--verbose", "--config", "file.toml"]
    ///   Output:
    /// - command_segments: [
    ///   { command: "scan", args: ["--since", "1week"] },
    ///   { command: "status", args: ["--format", "json"] }
    ///   ]
    pub fn segment_commands_only(
        &self,
        args: &[String],
        global_args: &[String],
    ) -> Result<Vec<CommandSegment>> {
        let mut command_segments = Vec::new();
        let mut current_command: Option<String> = None;
        let mut current_args = Vec::new();

        // Skip the global args and process the remaining arguments
        let remaining_args = &args[global_args.len()..];

        for arg in remaining_args {
            if self.is_known_command(arg) {
                // Save previous command segment if any
                if let Some(command_name) = current_command.take() {
                    command_segments.push(CommandSegment {
                        command_name,
                        args: std::mem::take(&mut current_args),
                    });
                }

                // Start new command segment
                current_command = Some(arg.clone());
            } else if current_command.is_some() {
                // We're in a command context, add to current args
                current_args.push(arg.clone());
            } else {
                // This shouldn't happen if global args were properly identified
                return Err(anyhow::anyhow!(
                    "Unexpected argument '{}' found after global args",
                    arg
                ));
            }
        }

        // Save final command segment if any
        if let Some(command_name) = current_command {
            command_segments.push(CommandSegment {
                command_name,
                args: current_args,
            });
        }

        Ok(command_segments)
    }

    /// Check if an argument is a known command
    fn is_known_command(&self, arg: &str) -> bool {
        self.known_commands.contains(&arg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_commands_only() {
        let segmenter =
            CommandSegmenter::with_commands(vec!["scan".to_string(), "status".to_string()]);
        let args = vec![
            "repostats".to_string(),
            "--verbose".to_string(),
            "--config".to_string(),
            "file.toml".to_string(),
            "scan".to_string(),
            "--since".to_string(),
            "1week".to_string(),
            "status".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        let global_args = vec![
            "repostats".to_string(),
            "--verbose".to_string(),
            "--config".to_string(),
            "file.toml".to_string(),
        ];

        let result = segmenter
            .segment_commands_only(&args, &global_args)
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].command_name, "scan");
        assert_eq!(result[0].args, vec!["--since", "1week"]);
        assert_eq!(result[1].command_name, "status");
        assert_eq!(result[1].args, vec!["--format", "json"]);
    }

    #[test]
    fn test_segment_commands_only_no_commands() {
        let segmenter = CommandSegmenter::with_commands(vec!["test".to_string()]);
        let args = vec!["repostats".to_string(), "--verbose".to_string()];
        let global_args = vec!["repostats".to_string(), "--verbose".to_string()];

        let result = segmenter
            .segment_commands_only(&args, &global_args)
            .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_segment_commands_only_single_command() {
        let segmenter = CommandSegmenter::with_commands(vec!["dump".to_string()]);
        let args = vec![
            "repostats".to_string(),
            "--log-level".to_string(),
            "debug".to_string(),
            "dump".to_string(),
            "--verbose".to_string(),
        ];
        let global_args = vec![
            "repostats".to_string(),
            "--log-level".to_string(),
            "debug".to_string(),
        ];

        let result = segmenter
            .segment_commands_only(&args, &global_args)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command_name, "dump");
        assert_eq!(result[0].args, vec!["--verbose"]);
    }
}
