//! Timeout wrapper around any future.

use std::time::Duration;

use thiserror::Error;

/// Returned when an operation does not complete within its deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("operation timed out after {0:?}")]
pub struct TimeoutError(pub Duration);

/// Run `fut`, failing with [`TimeoutError`] if it exceeds `dur`.
///
/// # Errors
/// Returns `Err(TimeoutError)` if the future does not resolve in time.
pub async fn with_timeout<F, T>(dur: Duration, fut: F) -> Result<T, TimeoutError>
where
    F: std::future::Future<Output = T>,
{
    match tokio::time::timeout(dur, fut).await {
        Ok(value) => Ok(value),
        Err(_) => Err(TimeoutError(dur)),
    }
}
