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

/// Represents the segmented command line arguments
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentedArgs {
    /// Global arguments (before any command)
    pub global_args: Vec<String>,
    /// Command-specific argument segments
    pub command_segments: Vec<CommandSegment>,
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

    /// Segment command line arguments by command boundaries
    ///
    /// Example input: ["--verbose", "--config", "file.toml", "scan", "--since", "1week", "status", "--format", "json"]
    /// Output:
    /// - global_args: ["--verbose", "--config", "file.toml"]
    /// - command_segments: [
    ///     { command: "scan", args: ["--since", "1week"] },
    ///     { command: "status", args: ["--format", "json"] }
    ///   ]
    ///
    pub fn segment_arguments(&self, args: &[String]) -> Result<SegmentedArgs> {
        let mut global_args = Vec::new();
        let mut command_segments = Vec::new();
        let mut current_command: Option<String> = None;
        let mut current_args = Vec::new();

        global_args.push(args[0].clone());
        let mut i = 1;
        while i < args.len() {
            let arg = &args[i];

            // Check if this is a known command
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
                // We're in global context
                global_args.push(arg.clone());
            }

            i += 1;
        }

        // Save final command segment if any
        if let Some(command_name) = current_command {
            command_segments.push(CommandSegment {
                command_name,
                args: current_args,
            });
        }

        Ok(SegmentedArgs {
            global_args,
            command_segments,
        })
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
    fn test_empty_args() {
        let segmenter = CommandSegmenter::with_commands(vec!["test".to_string()]);
        let result = segmenter
            .segment_arguments(&["repostats".to_string()])
            .unwrap();

        assert_eq!(result.global_args, vec!["repostats"]);
        assert_eq!(result.command_segments, Vec::<CommandSegment>::new());
    }

    #[test]
    fn test_global_args_only() {
        let segmenter = CommandSegmenter::with_commands(vec!["test".to_string()]);
        let args = vec![
            "repostats".to_string(),
            "--verbose".to_string(),
            "--config".to_string(),
            "file.toml".to_string(),
        ];

        let result = segmenter.segment_arguments(&args).unwrap();

        assert_eq!(
            result.global_args,
            vec!["repostats", "--verbose", "--config", "file.toml"]
        );
        assert!(result.command_segments.is_empty());
    }

    #[test]
    fn test_single_command_no_args() {
        let segmenter = CommandSegmenter::with_commands(vec!["status".to_string()]);
        let args = vec!["repostats".to_string(), "status".to_string()];

        let result = segmenter.segment_arguments(&args).unwrap();

        assert_eq!(result.global_args, vec!["repostats"]);
        assert_eq!(result.command_segments.len(), 1);
        assert_eq!(result.command_segments[0].command_name, "status");
        assert!(result.command_segments[0].args.is_empty());
    }

    #[test]
    fn test_single_command_with_args() {
        let segmenter = CommandSegmenter::with_commands(vec!["scan".to_string()]);
        let args = vec![
            "repostats".to_string(),
            "--verbose".to_string(),
            "scan".to_string(),
            "--since".to_string(),
            "1week".to_string(),
        ];

        let result = segmenter.segment_arguments(&args).unwrap();

        assert_eq!(result.global_args, vec!["repostats", "--verbose"]);
        assert_eq!(result.command_segments.len(), 1);
        assert_eq!(result.command_segments[0].command_name, "scan");
        assert_eq!(result.command_segments[0].args, vec!["--since", "1week"]);
    }

    #[test]
    fn test_multiple_commands() {
        let segmenter = CommandSegmenter::with_commands(vec![
            "scan".to_string(),
            "status".to_string(),
            "report".to_string(),
        ]);
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
            "report".to_string(),
            "--output".to_string(),
            "report.html".to_string(),
        ];

        let result = segmenter.segment_arguments(&args).unwrap();

        assert_eq!(
            result.global_args,
            vec!["repostats", "--verbose", "--config", "file.toml"]
        );
        assert_eq!(result.command_segments.len(), 3);

        assert_eq!(result.command_segments[0].command_name, "scan");
        assert_eq!(result.command_segments[0].args, vec!["--since", "1week"]);

        assert_eq!(result.command_segments[1].command_name, "status");
        assert_eq!(result.command_segments[1].args, vec!["--format", "json"]);

        assert_eq!(result.command_segments[2].command_name, "report");
        assert_eq!(
            result.command_segments[2].args,
            vec!["--output", "report.html"]
        );
    }

    #[test]
    fn test_command_like_args_not_treated_as_commands() {
        let segmenter = CommandSegmenter::with_commands(vec!["scan".to_string()]);
        let args = vec![
            "repostats".to_string(),
            "--mode".to_string(),
            "status".to_string(), // "status" is not a known command, so it's an arg
            "scan".to_string(),
            "--type".to_string(),
            "full".to_string(),
        ];

        let result = segmenter.segment_arguments(&args).unwrap();

        assert_eq!(result.global_args, vec!["repostats", "--mode", "status"]);
        assert_eq!(result.command_segments.len(), 1);
        assert_eq!(result.command_segments[0].command_name, "scan");
        assert_eq!(result.command_segments[0].args, vec!["--type", "full"]);
    }

    #[test]
    fn test_no_known_commands() {
        let segmenter = CommandSegmenter::with_commands(vec![]);
        let args = vec![
            "repostats".to_string(),
            "--verbose".to_string(),
            "unknown".to_string(),
            "--flag".to_string(),
        ];

        let result = segmenter.segment_arguments(&args).unwrap();

        assert_eq!(
            result.global_args,
            vec!["repostats", "--verbose", "unknown", "--flag"]
        );
        assert!(result.command_segments.is_empty());
    }

    #[test]
    fn test_consecutive_commands() {
        let segmenter =
            CommandSegmenter::with_commands(vec!["cmd1".to_string(), "cmd2".to_string()]);
        let args = vec![
            "repostats".to_string(),
            "cmd1".to_string(),
            "cmd2".to_string(),
            "--arg".to_string(),
        ];

        let result = segmenter.segment_arguments(&args).unwrap();

        assert_eq!(result.global_args, vec!["repostats"]);
        assert_eq!(result.command_segments.len(), 2);

        assert_eq!(result.command_segments[0].command_name, "cmd1");
        assert!(result.command_segments[0].args.is_empty());

        assert_eq!(result.command_segments[1].command_name, "cmd2");
        assert_eq!(result.command_segments[1].args, vec!["--arg"]);
    }

    #[test]
    fn test_command_with_equals_syntax() {
        let segmenter = CommandSegmenter::with_commands(vec!["scan".to_string()]);
        let args = vec![
            "repostats".to_string(),
            "--config=file.toml".to_string(),
            "scan".to_string(),
            "--since=1week".to_string(),
            "--format=json".to_string(),
        ];

        let result = segmenter.segment_arguments(&args).unwrap();

        assert_eq!(result.global_args, vec!["repostats", "--config=file.toml"]);
        assert_eq!(result.command_segments.len(), 1);
        assert_eq!(result.command_segments[0].command_name, "scan");
        assert_eq!(
            result.command_segments[0].args,
            vec!["--since=1week", "--format=json"]
        );
    }

    #[test]
    fn test_command_at_end() {
        let segmenter = CommandSegmenter::with_commands(vec!["help".to_string()]);
        let args = vec![
            "repostats".to_string(),
            "--verbose".to_string(),
            "--config".to_string(),
            "file.toml".to_string(),
            "help".to_string(),
        ];

        let result = segmenter.segment_arguments(&args).unwrap();

        assert_eq!(
            result.global_args,
            vec!["repostats", "--verbose", "--config", "file.toml"]
        );
        assert_eq!(result.command_segments.len(), 1);
        assert_eq!(result.command_segments[0].command_name, "help");
        assert!(result.command_segments[0].args.is_empty());
    }
}
