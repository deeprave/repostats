//! Command Line Segmentation
//!
//! This module handles the segmentation of command line arguments by command boundaries.
//! It splits the command line into global arguments and command-specific argument segments.
//! This is a pure, self-contained argument parser with no external dependencies or concerns.

use crate::app::startup::StartupError;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandSegment {
    /// Command name
    pub command_name: String,
    /// Arguments for this command
    pub args: Vec<String>,
}

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

    /// Check if an argument is a known command
    fn is_known_command(&self, arg: &str) -> bool {
        self.known_commands.contains(&arg.to_string())
    }

    /// Segment remaining arguments after global args have been identified
    ///
    pub fn segment_commands(&self, args: &[String]) -> Result<Vec<CommandSegment>, StartupError> {
        let mut command_segments = Vec::new();
        let mut current_command: Option<String> = None;
        let mut current_args = Vec::new();

        for arg in args {
            if self.is_known_command(arg) {
                // Previusly acccumulated command line
                if current_command.is_some() {
                    command_segments.push(CommandSegment {
                        command_name: current_command.unwrap(),
                        args: std::mem::take(&mut current_args),
                    });
                }
                // New command
                current_command = Some(arg.to_string());
                current_args = vec![arg.to_string()];
            } else if current_command.is_some() {
                // We're in a command context, add to current args
                current_args.push(arg.clone());
            } else {
                // command or arg not known
                return Err(StartupError::UnexpectedArgument { arg: arg.clone() });
            }
        }
        if current_command.is_some() {
            // wrap up the last command
            command_segments.push(CommandSegment {
                command_name: current_command.unwrap(),
                args: std::mem::take(&mut current_args),
            });
        }
        Ok(command_segments)
    }
}
