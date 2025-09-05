//! Synchronization utilities for robust mutex handling
//!
//! This module provides utilities for handling mutex poisoning and other
//! synchronization concerns in a consistent manner across the codebase.

use std::sync::{LockResult, RwLockReadGuard, RwLockWriteGuard};

/// Handle poisoned mutex cases with consistent error handling
///
/// This utility function converts mutex poison errors into application-specific
/// errors using a provided error constructor. This ensures consistent error
/// handling across the codebase when mutexes become poisoned due to panics.
///
/// # Arguments
/// * `result` - The result from a mutex lock operation
/// * `error_constructor` - Function to create the appropriate error type
///
/// # Returns
/// The mutex guard on success, or an application error on poison/failure
///
/// # Examples
/// ```
/// use std::sync::Mutex;
/// use repostats::core::sync::handle_mutex_poison;
/// use repostats::scanner::api::ScanError;
///
/// let mutex = Mutex::new(42);
/// let guard = handle_mutex_poison(
///     mutex.lock(),
///     |msg| ScanError::Configuration { message: msg }
/// ).unwrap();
/// ```
pub fn handle_mutex_poison<T, E>(
    result: LockResult<T>,
    error_constructor: impl FnOnce(String) -> E,
) -> Result<T, E> {
    result.map_err(|poison_err| {
        error_constructor(
            format!(
                "Internal synchronisation error (mutex poisoned). This indicates a panic occurred while holding a lock. PoisonError: {:?}",
                poison_err
            )
        )
    })
}

/// Handle poisoned RwLock read operations with consistent error handling
///
/// Similar to handle_mutex_poison but specifically for RwLock read operations.
/// RwLocks can become poisoned when a writer panics while holding the lock.
///
/// # Arguments
/// * `result` - The result from an RwLock read() operation
/// * `error_constructor` - Function to create the appropriate error type
///
/// # Returns
/// The RwLock read guard on success, or an application error on poison/failure
pub fn handle_rwlock_read<T, E>(
    result: LockResult<RwLockReadGuard<T>>,
    error_constructor: impl FnOnce(String) -> E,
) -> Result<RwLockReadGuard<T>, E> {
    result.map_err(|poison_err| {
        error_constructor(
            format!(
                "Internal synchronisation error (RwLock read poisoned). This indicates a panic occurred while holding a write lock. PoisonError: {:?}",
                poison_err
            )
        )
    })
}

/// Handle poisoned RwLock write operations with consistent error handling
///
/// Similar to handle_mutex_poison but specifically for RwLock write operations.
/// RwLocks can become poisoned when any thread holding the lock panics.
///
/// # Arguments
/// * `result` - The result from an RwLock write() operation
/// * `error_constructor` - Function to create the appropriate error type
///
/// # Returns
/// The RwLock write guard on success, or an application error on poison/failure
pub fn handle_rwlock_write<T, E>(
    result: LockResult<RwLockWriteGuard<T>>,
    error_constructor: impl FnOnce(String) -> E,
) -> Result<RwLockWriteGuard<T>, E> {
    result.map_err(|poison_err| {
        error_constructor(
            format!(
                "Internal synchronisation error (RwLock write poisoned). This indicates a panic occurred while holding the lock. PoisonError: {:?}",
                poison_err
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, RwLock};
    use std::thread;

    #[derive(Debug, PartialEq)]
    struct TestError {
        message: String,
    }

    #[test]
    fn test_handle_mutex_poison_success() {
        let mutex = Arc::new(Mutex::new(42));
        let result = handle_mutex_poison(mutex.lock(), |msg| TestError { message: msg });

        assert!(result.is_ok());
        assert_eq!(*result.unwrap(), 42);
    }

    #[test]
    fn test_handle_mutex_poison_with_poisoned_mutex() {
        let mutex = Arc::new(Mutex::new(42));
        let mutex_clone = Arc::clone(&mutex);

        // Poison the mutex by panicking while holding the lock
        let _ = thread::spawn(move || {
            let _guard = mutex_clone.lock().unwrap();
            panic!("Intentional panic to poison mutex");
        })
        .join();

        // Now try to lock the poisoned mutex
        let result = handle_mutex_poison(mutex.lock(), |msg| TestError { message: msg });

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("mutex poisoned"));
        assert!(error.message.contains("panic occurred"));
    }

    #[test]
    fn test_handle_rwlock_read_success() {
        let rwlock = RwLock::new(42);

        let result = handle_rwlock_read(rwlock.read(), |msg| TestError { message: msg });

        // Should succeed and return the value
        assert!(result.is_ok());
        assert_eq!(*result.unwrap(), 42);
    }

    #[test]
    fn test_handle_rwlock_write_success() {
        let rwlock = RwLock::new(42);

        let result = handle_rwlock_write(rwlock.write(), |msg| TestError { message: msg });

        // Should succeed and return the guard
        assert!(result.is_ok());
        *result.unwrap() = 100;
        assert_eq!(*rwlock.read().unwrap(), 100);
    }
}
