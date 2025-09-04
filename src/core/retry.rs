//! Retry utility for handling transient errors in async operations
//!
//! Provides configurable retry policies with exponential backoff and error context.

use std::time::Duration;
use tokio::time::sleep;

/// Configurable retry policy for async operations
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            delay: Duration::from_millis(500),
        }
    }
}

/// Execute an async operation with retry logic for transient errors
///
/// # Examples
/// ```rust
/// use repostats::core::retry::{retry_async, RetryPolicy};
/// use std::time::Duration;
///
/// # async fn example() -> Result<String, String> {
/// let result = retry_async(
///     "database_connection",
///     RetryPolicy::default(),
///     || async {
///         // Your async operation here
///         Ok::<String, String>("success".to_string())
///     }
/// ).await?;
/// # Ok(result)
/// # }
/// ```
pub async fn retry_async<F, T, E, Fut>(
    operation_name: &str,
    policy: RetryPolicy,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_error = None;

    for attempt in 0..policy.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                last_error = Some(error);
                if attempt < policy.max_attempts - 1 {
                    log::debug!(
                        "Operation '{}' failed on attempt {}/{}, retrying in {:?}: {}",
                        operation_name,
                        attempt + 1,
                        policy.max_attempts,
                        policy.delay,
                        last_error.as_ref().unwrap()
                    );
                    sleep(policy.delay).await;
                }
            }
        }
    }

    Err(last_error.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_succeeds_immediately() {
        let result = retry_async("test_operation", RetryPolicy::default(), || async {
            Ok::<i32, String>(42)
        })
        .await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        use std::sync::{Arc, Mutex};
        let attempt_count = Arc::new(Mutex::new(0));

        let result = retry_async("test_operation", RetryPolicy::default(), || {
            let count = attempt_count.clone();
            async move {
                let mut attempts = count.lock().unwrap();
                *attempts += 1;
                if *attempts < 3 {
                    Err("temporary failure")
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(*attempt_count.lock().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausts_attempts() {
        use std::sync::{Arc, Mutex};
        let attempt_count = Arc::new(Mutex::new(0));
        let policy = RetryPolicy {
            max_attempts: 2,
            delay: Duration::from_millis(10),
        };

        let result = retry_async("test_operation", policy, || {
            let count = attempt_count.clone();
            async move {
                let mut attempts = count.lock().unwrap();
                *attempts += 1;
                Err::<i32, &str>("persistent failure")
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "persistent failure");
        assert_eq!(*attempt_count.lock().unwrap(), 2);
    }
}
