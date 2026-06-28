//! Errors for the optional networked advisor backends.

use thiserror::Error;

/// Failures that can occur while calling an external LLM. These are never
/// surfaced to callers of [`crate::RiskAdvisor::assess`] — they trigger a
/// fallback to the deterministic heuristic instead.
#[derive(Debug, Error)]
pub enum AiError {
    /// The HTTP request failed (DNS, TCP, TLS, or status).
    #[error("llm transport error: {0}")]
    Transport(String),
    /// The request exceeded its deadline.
    #[error("llm request timed out")]
    Timeout,
    /// The response body could not be parsed into the expected shape.
    #[error("llm response was malformed: {0}")]
    Malformed(String),
    /// The backend is disabled (e.g. no API key configured).
    #[error("llm backend disabled")]
    Disabled,
}

impl AiError {
    /// Whether a retry could plausibly succeed.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, AiError::Transport(_) | AiError::Timeout)
    }
}
