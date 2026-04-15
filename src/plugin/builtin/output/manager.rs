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

/// Stateful output pipeline for a single output plugin execution.
///
/// The pipeline caches the formatter and writer so template loading and file
/// initialization happen once per plugin execution, not once per export event.
pub struct OutputPipeline {
    formatter: Box<dyn OutputFormatter>,
    writer: Box<dyn OutputWriter>,
    destination_description: String,
    format_name: &'static str,
    use_colors: bool,
    has_written: bool,
}

impl OutputPipeline {
    pub async fn new(config: OutputConfig) -> PluginResult<Self> {
        let formatter = Self::create_formatter(&config).await?;
        let format_name = formatter.format_type().name();
        let writer = Self::create_writer(&config)?;
        let destination_description = Self::describe_destination(&config.destination);

        Ok(Self {
            formatter,
            writer,
            destination_description,
            format_name,
            use_colors: config.use_colors,
            has_written: false,
        })
    }

    fn create_writer(config: &OutputConfig) -> PluginResult<Box<dyn OutputWriter>> {
        match &config.destination {
            OutputDestination::Stdout => Ok(Box::new(StdoutWriter::new())),
            OutputDestination::File(path) => Ok(Box::new(FileWriter::new(path)?)),
        }
    }

    async fn create_formatter(config: &OutputConfig) -> PluginResult<Box<dyn OutputFormatter>> {
        match (&config.format, config.template_path.as_deref()) {
            (ExportFormat::Template, Some(source)) => {
                Ok(Box::new(TemplateFormatter::from_source(source).await?))
            }
            (format, _) => Ok(get_formatter(format.clone())),
        }
    }

    fn describe_destination(destination: &OutputDestination) -> String {
        match destination {
            OutputDestination::Stdout => "stdout".to_string(),
            OutputDestination::File(path) => path.clone(),
        }
    }

    pub fn export(&mut self, data: &PluginDataExport) -> PluginResult<()> {
        let content = self.formatter.format(data, self.use_colors)?;

        if self.has_written {
            self.writer
                .write_all(b"\n")
                .map_err(|e| PluginError::IoError {
                    operation: "separate output blocks".to_string(),
                    path: self.destination_description.clone(),
                    source: Some(Box::new(e)),
                })?;
        }

        self.writer
            .write_all(content.as_bytes())
            .map_err(|e| PluginError::IoError {
                operation: "write output".to_string(),
                path: self.destination_description.clone(),
                source: Some(Box::new(e)),
            })?;

        if !content.ends_with('\n') {
            self.writer
                .write_all(b"\n")
                .map_err(|e| PluginError::IoError {
                    operation: "terminate output line".to_string(),
                    path: self.destination_description.clone(),
                    source: Some(Box::new(e)),
                })?;
        }

        self.writer.finalize().map_err(|e| PluginError::IoError {
            operation: "finalize output".to_string(),
            path: self.destination_description.clone(),
            source: Some(Box::new(e)),
        })?;

        self.has_written = true;
        Ok(())
    }

    pub fn format_name(&self) -> &'static str {
        self.format_name
    }

    pub fn destination_description(&self) -> &str {
        &self.destination_description
    }
}
