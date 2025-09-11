//! Generic Shutdown Coordination
//!
//! Provides a generic, reusable shutdown coordination system that handles
//! signal handling and allows guarding code execution with coordinated shutdown.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Coordinates graceful shutdown across the application
pub struct ShutdownCoordinator {
    pub shutdown_tx: broadcast::Sender<()>,
    pub shutdown_requested: Arc<AtomicBool>,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator
    pub fn new() -> (Self, broadcast::Receiver<()>) {
        // Use a larger channel to avoid dropping bursts of shutdown signals
        let (shutdown_tx, shutdown_rx) = broadcast::channel(8);
        let shutdown_requested = Arc::new(AtomicBool::new(false));

        let coordinator = Self {
            shutdown_tx,
            shutdown_requested,
        };

        (coordinator, shutdown_rx)
    }

    /// Subscribe to shutdown notifications
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Trigger shutdown
    pub fn trigger_shutdown(&self) {
        // Use Release ordering to synchronize-with all Acquire loads
        // This ensures that any thread checking is_shutdown_requested()
        // will see this store and any previous memory operations
        self.shutdown_requested.store(true, Ordering::Release);
        let _ = self.shutdown_tx.send(());
    }

    /// Check if shutdown has been requested
    pub fn is_shutdown_requested(&self) -> bool {
        // Use Acquire ordering to synchronize-with Release stores
        // This ensures we see the most up-to-date shutdown state
        self.shutdown_requested.load(Ordering::Acquire)
    }

    /// Guard execution of a future with shutdown coordination
    ///
    /// This method automatically sets up signal handlers and coordinates
    /// shutdown for the provided closure, making it appear as if the
    /// closure is "guarded" by shutdown coordination.
    pub async fn guard<F, Fut, R, E>(future_fn: F) -> Result<R, E>
    where
        F: FnOnce(broadcast::Receiver<()>) -> Fut,
        Fut: std::future::Future<Output = Result<R, E>>,
    {
        let (coordinator, shutdown_rx) = Self::new();

        // Set up signal handlers automatically
        setup_signal_handlers(
            coordinator.shutdown_tx.clone(),
            coordinator.shutdown_requested.clone(),
        );

        // Execute the guarded code with shutdown receiver
        future_fn(shutdown_rx).await
    }

    /// Guard execution of a future with shutdown coordination, providing access to coordinator
    ///
    /// This variant gives the closure access to the coordinator for more complex scenarios
    pub async fn guard_with_coordinator<F, Fut, R, E>(future_fn: F) -> Result<R, E>
    where
        F: FnOnce(Self, broadcast::Receiver<()>) -> Fut,
        Fut: std::future::Future<Output = Result<R, E>>,
    {
        let (coordinator, shutdown_rx) = Self::new();

        // Set up signal handlers automatically
        setup_signal_handlers(
            coordinator.shutdown_tx.clone(),
            coordinator.shutdown_requested.clone(),
        );

        // Execute the guarded code with coordinator and shutdown receiver
        future_fn(coordinator, shutdown_rx).await
    }
}

/// Set up signal handlers for graceful shutdown
fn setup_signal_handlers(shutdown_tx: broadcast::Sender<()>, shutdown_requested: Arc<AtomicBool>) {
    #[cfg(unix)]
    {
        unsafe {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }

        use std::sync::atomic::AtomicUsize;
        use tokio::signal::unix::{signal, SignalKind};
        let signal_count = Arc::new(AtomicUsize::new(0));
        let signals = [
            SignalKind::interrupt(),
            SignalKind::terminate(),
            SignalKind::hangup(),
            SignalKind::quit(),
        ];

        for kind in signals {
            let tx = shutdown_tx.clone();
            let requested = shutdown_requested.clone();
            let sig_ctr = signal_count.clone();

            tokio::spawn(async move {
                if let Ok(mut sig) = signal(kind) {
                    #[allow(clippy::never_loop)]
                    // Intentional - circuit breaker pattern for signal handling
                    while sig.recv().await.is_some() {
                        let prev = sig_ctr.fetch_add(1, Ordering::AcqRel);
                        requested.store(true, Ordering::Release);
                        let _ = tx.send(());
                        if prev >= 1 {
                            // Second signal received; forcing immediate exit
                            std::process::exit(130);
                        }
                        // After first signal break to avoid busy loop for same kind
                        break;
                    }
                }
            });
        }

        // Fallback generic ctrl_c handler (covers terminals where specific UNIX signals not delivered as expected)
        {
            let tx = shutdown_tx.clone();
            let requested = shutdown_requested.clone();
            let sig_ctr = signal_count.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    let prev = sig_ctr.fetch_add(1, Ordering::AcqRel);
                    requested.store(true, Ordering::Release);
                    let _ = tx.send(());
                    if prev >= 1 {
                        log::warn!("Ctrl-C received; exiting");
                        std::process::exit(130);
                    }
                }
            });
        }
    }

    #[cfg(not(unix))]
    {
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                shutdown_requested.store(true, Ordering::Release);
                let _ = shutdown_tx.send(());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_shutdown_coordinator_creation() {
        let (coordinator, _rx) = ShutdownCoordinator::new();

        // Should start with shutdown not requested
        assert!(!coordinator.is_shutdown_requested());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_trigger() {
        let (coordinator, mut rx) = ShutdownCoordinator::new();

        // Initially shutdown should not be requested
        assert!(!coordinator.is_shutdown_requested());

        // Trigger shutdown
        coordinator.trigger_shutdown();

        // Should now report shutdown requested
        assert!(coordinator.is_shutdown_requested());

        // Should receive shutdown signal
        let signal_received = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(signal_received.is_ok(), "Should receive shutdown signal");
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_multiple_subscribers() {
        let (coordinator, _rx1) = ShutdownCoordinator::new();
        let mut rx2 = coordinator.subscribe();
        let mut rx3 = coordinator.subscribe();

        // Trigger shutdown
        coordinator.trigger_shutdown();

        // All subscribers should receive the signal
        let signal2 = timeout(Duration::from_millis(100), rx2.recv()).await;
        let signal3 = timeout(Duration::from_millis(100), rx3.recv()).await;

        assert!(
            signal2.is_ok(),
            "Subscriber 2 should receive shutdown signal"
        );
        assert!(
            signal3.is_ok(),
            "Subscriber 3 should receive shutdown signal"
        );
        assert!(coordinator.is_shutdown_requested());
    }

    #[tokio::test]
    async fn test_guard_functionality() {
        let result = ShutdownCoordinator::guard(|mut shutdown_rx| async move {
            // Simulate some work
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    Ok::<i32, &str>(42)
                }
                _ = shutdown_rx.recv() => {
                    Ok(-1)
                }
            }
        })
        .await;

        assert_eq!(result, Ok(42));
    }
}
