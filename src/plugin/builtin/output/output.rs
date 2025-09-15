//! Output destination handling for OutputPlugin

use crate::plugin::builtin::output::args::OutputConfig;
use crate::plugin::builtin::output::traits::{OutputDestination, OutputWriter};
use crate::plugin::error::{PluginError, PluginResult};
use std::fs::OpenOptions;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// Standard output writer
pub struct StdoutWriter {
    writer: io::Stdout,
}

impl StdoutWriter {
    /// Create a new stdout writer
    pub fn new() -> Self {
        Self {
            writer: io::stdout(),
        }
    }
}

impl Write for StdoutWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl OutputWriter for StdoutWriter {
    fn finalize(&mut self) -> io::Result<()> {
        self.flush()
    }
}

/// File output writer with buffering
pub struct FileWriter {
    writer: BufWriter<std::fs::File>,
    path: String,
}

impl FileWriter {
    /// Create a new file writer
    pub fn new(path: &str) -> PluginResult<Self> {
        // Validate path
        let file_path = Path::new(path);

        // Check if parent directory exists or can be created
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| PluginError::IoError {
                    operation: "create parent directory".to_string(),
                    path: parent.to_string_lossy().to_string(),
                    cause: e.to_string(),
                })?;
            }
        }

        // Open file for writing (create or truncate)
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)
            .map_err(|e| PluginError::IoError {
                operation: "create output file".to_string(),
                path: path.to_string(),
                cause: e.to_string(),
            })?;

        Ok(Self {
            writer: BufWriter::new(file),
            path: path.to_string(),
        })
    }
}

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl OutputWriter for FileWriter {
    fn finalize(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

/// Output manager for handling different output destinations
pub struct OutputManager {
    config: OutputConfig,
}

impl OutputManager {
    /// Create a new output manager with the given configuration
    pub fn new(config: OutputConfig) -> Self {
        Self { config }
    }

    /// Create an output writer based on the configuration
    pub fn create_writer(&self) -> PluginResult<Box<dyn OutputWriter>> {
        match &self.config.destination {
            OutputDestination::Stdout => Ok(Box::new(StdoutWriter::new())),
            OutputDestination::File(path) => Ok(Box::new(FileWriter::new(path)?)),
        }
    }

    /// Write formatted content to the configured destination
    pub fn write_output(&self, content: &str) -> PluginResult<()> {
        let mut writer = self.create_writer()?;

        writer
            .write_all(content.as_bytes())
            .map_err(|e| PluginError::IoError {
                operation: "write output".to_string(),
                path: self.get_destination_description(),
                cause: e.to_string(),
            })?;

        writer.finalize().map_err(|e| PluginError::IoError {
            operation: "finalize output".to_string(),
            path: self.get_destination_description(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    /// Get a description of the output destination for error messages
    fn get_destination_description(&self) -> String {
        match &self.config.destination {
            OutputDestination::Stdout => "stdout".to_string(),
            OutputDestination::File(path) => path.clone(),
        }
    }

    /// Get the output configuration
    pub fn config(&self) -> &OutputConfig {
        &self.config
    }

    /// Validate that the output destination is writable
    pub fn validate_destination(&self) -> PluginResult<()> {
        match &self.config.destination {
            OutputDestination::Stdout => Ok(()), // stdout is always available
            OutputDestination::File(path) => {
                let file_path = Path::new(path);

                // Check if we can write to the parent directory
                if let Some(parent) = file_path.parent() {
                    if parent.exists()
                        && parent
                            .metadata()
                            .map(|m| m.permissions().readonly())
                            .unwrap_or(false)
                    {
                        return Err(PluginError::IoError {
                            operation: "validate output destination".to_string(),
                            path: path.clone(),
                            cause: "Parent directory is read-only".to_string(),
                        });
                    }
                }

                // Try to create a test file to verify we can write
                if let Err(e) = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(file_path)
                    .and_then(|_| std::fs::remove_file(file_path).or(Ok(())))
                {
                    return Err(PluginError::IoError {
                        operation: "validate output destination".to_string(),
                        path: path.clone(),
                        cause: e.to_string(),
                    });
                }

                Ok(())
            }
        }
    }
}
