//! Output destination handling for OutputPlugin

use crate::plugin::builtin::output::args::OutputConfig;
use crate::plugin::builtin::output::formats::{get_formatter, template::TemplateFormatter};
use crate::plugin::builtin::output::traits::{
    ExportFormat, OutputDestination, OutputFormatter, OutputWriter,
};
use crate::plugin::data_export::PluginDataExport;
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
                    source: Some(Box::new(e)),
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
                source: Some(Box::new(e)),
            })?;

        Ok(Self {
            writer: BufWriter::new(file),
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

    async fn create_formatter(&self) -> PluginResult<Box<dyn OutputFormatter>> {
        match (&self.config.format, self.config.template_path.as_deref()) {
            (ExportFormat::Template, Some(source)) => {
                Ok(Box::new(TemplateFormatter::from_source(source).await?))
            }
            (format, _) => Ok(get_formatter(format.clone())),
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
                source: Some(Box::new(e)),
            })?;

        writer.finalize().map_err(|e| PluginError::IoError {
            operation: "finalize output".to_string(),
            path: self.get_destination_description(),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    /// Render and write a plugin export using the configured formatter and destination.
    pub async fn export(&self, data: &PluginDataExport) -> PluginResult<()> {
        let formatter = self.create_formatter().await?;
        let format_name = formatter.format_type().name();
        let content = formatter.format(data, self.config.use_colors)?;
        self.write_output(&content)?;

        log::debug!(
            "Rendered output using '{}' formatter to {}",
            format_name,
            self.get_destination_description()
        );

        Ok(())
    }

    /// Get a description of the output destination for error messages
    fn get_destination_description(&self) -> String {
        match &self.config.destination {
            OutputDestination::Stdout => "stdout".to_string(),
            OutputDestination::File(path) => path.clone(),
        }
    }
}
