//! Error types for the risk engine and its ports.

use marginguard_types::InvalidInput;
use thiserror::Error;

/// A failure originating from an outbound port (store, oracle, sink).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PortError {
    /// The dependency is temporarily unavailable.
    #[error("dependency unavailable: {0}")]
    Unavailable(String),
    /// The dependency timed out.
    #[error("dependency timed out")]
    Timeout,
    /// An internal/unexpected failure.
    #[error("internal error: {0}")]
    Internal(String),
}

impl PortError {
    /// Whether a retry could plausibly succeed.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, PortError::Unavailable(_) | PortError::Timeout)
    }
}

/// The top-level engine error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// An invalid input value.
    #[error(transparent)]
    Invalid(#[from] InvalidInput),
    /// A port failure.
    #[error(transparent)]
    Port(#[from] PortError),
    /// The referenced position does not exist.
    #[error("position not found")]
    PositionNotFound,
    /// A position already exists for this account/market.
    #[error("position already exists")]
    PositionExists,
    /// The referenced market has no state yet.
    #[error("market not found")]
    MarketNotFound,
    /// Posted margin was below the initial-margin requirement.
    #[error("insufficient initial margin")]
    InsufficientMargin,
    /// Command ingestion was throttled by the rate limiter.
    #[error("rate limited")]
    RateLimited,
}

impl CoreError {
    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::Invalid(e) => e.code(),
            CoreError::Port(_) => "port_error",
            CoreError::PositionNotFound => "position_not_found",
            CoreError::PositionExists => "position_exists",
            CoreError::MarketNotFound => "market_not_found",
            CoreError::InsufficientMargin => "insufficient_margin",
            CoreError::RateLimited => "rate_limited",
        }
    }
}
