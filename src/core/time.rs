//! Time provider abstraction for testable time-dependent logic

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use std::time::Duration;
use std::time::{Instant, SystemTime};

/// Abstraction over system time for testable time-dependent logic
#[allow(dead_code)]
pub trait TimeProvider: Send + Sync {
    /// Get the current monotonic time (for measuring intervals)
    fn now(&self) -> Instant;

    /// Get the current system time (for timestamps)
    fn system_time(&self) -> SystemTime;
}

/// Production time provider using actual system time
#[derive(Default, Clone)]
pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn system_time(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Mock time provider for deterministic testing
#[derive(Clone)]
#[cfg(test)]
pub struct MockTimeProvider {
    current_instant: Arc<Mutex<Instant>>,
    current_system_time: Arc<Mutex<SystemTime>>,
}

#[cfg(test)]
impl MockTimeProvider {
    fn default() -> Self {
        Self::new()
    }
    /// Create a new mock time provider starting at the given time
    pub fn new() -> Self {
        let base_instant = Instant::now();
        let base_system_time = SystemTime::now();

        Self {
            current_instant: Arc::new(Mutex::new(base_instant)),
            current_system_time: Arc::new(Mutex::new(base_system_time)),
        }
    }

    /// Advance both monotonic and system time by the given duration
    pub fn advance_time(&self, duration: Duration) {
        {
            let mut instant = self.current_instant.lock().unwrap();
            *instant += duration;
        }
        {
            let mut system_time = self.current_system_time.lock().unwrap();
            *system_time += duration;
        }
    }

    /// Set the current instant time (for interval measurements)
    pub fn set_instant(&self, instant: Instant) {
        let mut current = self.current_instant.lock().unwrap();
        *current = instant;
    }

    /// Set the current system time (for timestamps)
    pub fn set_system_time(&self, system_time: SystemTime) {
        let mut current = self.current_system_time.lock().unwrap();
        *current = system_time;
    }
}

#[cfg(test)]
impl TimeProvider for MockTimeProvider {
    fn now(&self) -> Instant {
        *self.current_instant.lock().unwrap()
    }

    fn system_time(&self) -> SystemTime {
        *self.current_system_time.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_system_time_provider() {
        let provider = SystemTimeProvider::default();

        let instant1 = provider.now();
        let system1 = provider.system_time();

        std::thread::sleep(Duration::from_millis(1));

        let instant2 = provider.now();
        let system2 = provider.system_time();

        assert!(instant2 > instant1);
        assert!(system2 > system1);
    }

    #[test]
    fn test_mock_time_provider() {
        let provider = MockTimeProvider::new();

        let initial_instant = provider.now();
        let initial_system = provider.system_time();

        // Advance time
        provider.advance_time(Duration::from_secs(10));

        let after_instant = provider.now();
        let after_system = provider.system_time();

        assert_eq!(
            after_instant.duration_since(initial_instant),
            Duration::from_secs(10)
        );
        assert_eq!(
            after_system.duration_since(initial_system).unwrap(),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn test_mock_time_provider_set_times() {
        let provider = MockTimeProvider::new();

        let base_instant = Instant::now();
        let base_system = SystemTime::now();

        provider.set_instant(base_instant);
        provider.set_system_time(base_system);

        assert_eq!(provider.now(), base_instant);
        assert_eq!(provider.system_time(), base_system);
    }
}
