//! Bounded retry with deterministic equal-jitter exponential backoff.
//!
//! Backoff uses no RNG: the jitter is derived deterministically from the
//! attempt number, so retries are reproducible in tests and traces.

use std::time::Duration;

/// Retry configuration.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Maximum number of attempts (>= 1).
    pub max_attempts: u32,
    /// Base delay used for the first backoff.
    pub base_delay: Duration,
    /// Upper bound on any single backoff.
    pub max_delay: Duration,
}

impl RetryPolicy {
    /// A policy of `attempts` tries with the given base delay.
    #[must_use]
    pub const fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        RetryPolicy {
            max_attempts,
            base_delay,
            max_delay,
        }
    }

    /// Deterministic equal-jitter backoff for a zero-based attempt index.
    #[must_use]
    pub fn backoff(&self, attempt: u32) -> Duration {
        let exp = self.base_delay.saturating_mul(1u32 << attempt.min(16));
        let capped = exp.min(self.max_delay);
        // Equal jitter: half fixed + half a deterministic fraction of the half.
        let half = capped / 2;
        let jitter_steps = (u64::from(attempt) * 2_654_435_761) % 1_000;
        let jitter = half * u32::try_from(jitter_steps).unwrap_or(0) / 1_000;
        half + jitter
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy::new(3, Duration::from_millis(50), Duration::from_secs(2))
    }
}

/// Run `op` up to `policy.max_attempts` times, retrying while `is_retryable`
/// returns true, sleeping `policy.backoff` between attempts.
///
/// # Errors
/// Returns the last error if all attempts fail or the error is non-retryable.
pub async fn retry<F, Fut, T, E>(
    policy: RetryPolicy,
    is_retryable: impl Fn(&E) -> bool,
    mut op: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut attempt = 0;
    loop {
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                attempt += 1;
                if attempt >= policy.max_attempts || !is_retryable(&err) {
                    return Err(err);
                }
                tokio::time::sleep(policy.backoff(attempt - 1)).await;
            }
        }
    }
}
