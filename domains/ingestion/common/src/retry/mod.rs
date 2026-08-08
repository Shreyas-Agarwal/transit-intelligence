//! Generic async retry-with-backoff.
//!
//! Kept deliberately generic (`FnMut() -> Future<Output = Result<T, E>>`) so it has
//! no opinion on what's being retried — a CKAN API call, a chunk of a download, etc.
//! Callers decide what counts as retryable by returning `Err` only for conditions
//! that should be retried; anything that shouldn't be retried should be handled
//! before this function sees it (e.g. by returning `Ok` with an error payload, or
//! by not calling `retry_async` at all for non-idempotent operations).

use std::future::Future;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Total number of attempts, including the first (non-retry) one.
    pub max_attempts: usize,
    pub base_delay: Duration,
    /// Backoff multiplier applied after each failed attempt.
    pub multiplier: f64,
    pub max_delay: Duration,
}

impl RetryPolicy {
    pub fn new(max_attempts: usize, base_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
            multiplier: 2.0,
            max_delay: Duration::from_secs(30),
        }
    }

    fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let scaled = self.base_delay.as_secs_f64() * self.multiplier.powi(attempt as i32);
        Duration::from_secs_f64(scaled.min(self.max_delay.as_secs_f64()))
    }
}

/// Runs `f` up to `policy.max_attempts` times, sleeping with exponential backoff
/// between failures. Returns the last error if every attempt fails.
pub async fn retry_async<F, Fut, T, E>(policy: RetryPolicy, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;
    loop {
        match f().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                attempt += 1;
                if attempt >= policy.max_attempts {
                    return Err(err);
                }
                let delay = policy.delay_for_attempt(attempt - 1);
                tracing::warn!(
                    attempt,
                    max_attempts = policy.max_attempts,
                    delay_ms = delay.as_millis() as u64,
                    error = %err,
                    "retrying after failure"
                );
                tokio::time::sleep(delay).await;
            }
        }
    }
}
