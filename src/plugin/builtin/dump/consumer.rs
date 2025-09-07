//! Consumer loop logic for DumpPlugin
use super::{DumpPlugin, OutputFormat};
use crate::plugin::error::PluginResult;
use crate::plugin::traits::Plugin; // for plugin_info()
use crate::queue::api::{QueueConsumer, QueueError};
use crate::queue::typed::{TypedMessage, TypedQueueConsumer};
use crate::scanner::api::ScanMessage;
use log::error;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

impl DumpPlugin {
    pub(super) async fn start_consumer_loop(
        &mut self,
        consumer: QueueConsumer,
    ) -> PluginResult<()> {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        let typed_consumer = TypedQueueConsumer::<ScanMessage>::new(consumer);

        let output_format = self.output_format;
        let show_headers = self.show_headers;
        let send_keepalive = self.send_keepalive;
        let keepalive_interval_secs = self.keepalive_interval_secs;
        let plugin_name = self.plugin_info().name;
        let use_colors = self.use_colors;
        let outfile_path = self.output_file.clone();

        let task_handle = tokio::spawn(async move {
            // Create thread-safe writer if output file is specified
            let file_writer = if let Some(path) = &outfile_path {
                match std::fs::File::create(path) {
                    Ok(f) => Some(Arc::new(Mutex::new(std::io::BufWriter::new(f)))),
                    Err(e) => {
                        error!("Failed to create output file {:?}: {}", path, e);
                        None
                    }
                }
            } else {
                None
            };

            let mut message_count = 0usize;
            let mut active_scanners = HashSet::new();
            let mut completed_scanners = HashSet::new();
            let mut consecutive_keepalive_failures = 0usize;
            const MAX_FAIL: usize = 3;
            let mut keepalive_interval = if send_keepalive {
                Some(tokio::time::interval(tokio::time::Duration::from_secs(
                    keepalive_interval_secs,
                )))
            } else {
                None
            };
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => { break; }
                    _ = async { if let Some(ref mut i)=keepalive_interval { i.tick().await; } else { std::future::pending::<()>().await } } => {
                        if let Err(_e) = crate::plugin::events::publish_plugin_keepalive_event(&plugin_name, &format!("Processed {} messages", message_count)).await { consecutive_keepalive_failures +=1; if consecutive_keepalive_failures>=MAX_FAIL { keepalive_interval=None; } } else { consecutive_keepalive_failures=0; }
                    }
                    result = tokio::time::timeout(tokio::time::Duration::from_millis(1000), async { typed_consumer.read_with_header() }) => {
                        match result {
                            Ok(Ok(Some(typed_msg))) => {
                                Self::handle_message(&plugin_name, &typed_msg, output_format, show_headers, use_colors, &file_writer);
                                message_count+=1;
                                match &typed_msg.content {
                                    ScanMessage::ScanStarted {
                                        scanner_id, ..
                                    } => {
                                        active_scanners.insert(scanner_id.clone());
                                    },
                                    ScanMessage::ScanCompleted {
                                        scanner_id, ..
                                    } => {
                                        completed_scanners.insert(scanner_id.clone());
                                        // If we have active scanners, check if all are completed
                                        // If no active scanners were registered, treat completion as end signal
                                        if active_scanners.is_empty() || active_scanners.iter().all(|id| completed_scanners.contains(id)) {
                                            let _ = crate::plugin::events::publish_plugin_completion_event(&plugin_name, "All scanners completed - plugin processing finished").await;
                                            break;
                                        }
                                    },
                                    ScanMessage::ScanError {
                                        scanner_id, ..
                                    } => {
                                        completed_scanners.insert(scanner_id.clone());
                                        // If we have active scanners, check if all are completed
                                        // If no active scanners were registered, treat error as end signal
                                        if active_scanners.is_empty() || active_scanners.iter().all(|id| completed_scanners.contains(id)) {
                                            let _ = crate::plugin::events::publish_plugin_completion_event(&plugin_name, "All scanners completed - plugin processing finished").await;
                                            break;
                                        }
                                        let _ = crate::plugin::events::publish_plugin_error_event(&plugin_name, "Unexpected scan error").await;
                                    }, _ => {}
                                }
                            }
                            Ok(Ok(None)) => {}
                            Ok(Err(e)) => { if let QueueError::DeserializationError { message } = &e { error!("DumpPlugin: deserialization failed: {}", message); } }
                            Err(_) => {}
                        }
                    }
                }
            }
        });
        self.shutdown_tx = Some(shutdown_tx);
        self.consumer_task = Some(task_handle);
        Ok(())
    }

    fn handle_message(
        _plugin_name: &str,
        typed_msg: &TypedMessage<ScanMessage>,
        output_format: OutputFormat,
        show_headers: bool,
        use_colors: bool,
        file_writer: &Option<Arc<Mutex<std::io::BufWriter<std::fs::File>>>>,
    ) {
        let formatted = match output_format {
            OutputFormat::Json => {
                super::format::format_json_typed(typed_msg, show_headers, use_colors)
            }
            OutputFormat::Compact => {
                super::format::format_compact_typed(typed_msg, show_headers, use_colors)
            }
            OutputFormat::Raw => {
                super::format::format_text_typed(typed_msg, show_headers, use_colors)
            }
            OutputFormat::Text => {
                super::format::format_pretty_text_typed(typed_msg, show_headers, use_colors)
            }
        };
        if let Some(writer) = file_writer {
            use std::io::Write;
            match writer.lock() {
                Ok(mut w) => {
                    if let Err(e) = writeln!(w, "{}", formatted) {
                        error!("Failed to write message to output file: {}", e);
                    }
                    if let Err(e) = w.flush() {
                        error!("Failed to flush output file: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to acquire lock on output file writer: {}", e);
                }
            }
        } else {
            println!("{}", formatted);
        }
    }

    pub(super) async fn stop_consumer_loop(&mut self) -> PluginResult<()> {
        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Wait for consumer task to complete
        if let Some(task_handle) = self.consumer_task.take() {
            match task_handle.await {
                Ok(()) => {
                    // Task completed successfully
                }
                Err(e) if e.is_cancelled() => {
                    // Task was cancelled, this is expected during shutdown
                }
                Err(e) => {
                    error!("Consumer task failed: {}", e);
                }
            }
        }

        Ok(())
    }
}
