//! # marginguard-resilience
//!
//! Small, dependency-light fault-tolerance primitives shared across MarginGuard
//! services: an injectable [`Clock`], a [`with_timeout`] combinator, bounded
//! [`retry`] with deterministic backoff, a [`RateLimiter`], and a
//! [`CircuitBreaker`]. Every time-dependent type is generic over the clock so
//! tests run without real sleeping.

#![forbid(unsafe_code)]

pub mod breaker;
pub mod clock;
pub mod limiter;
pub mod retry;
pub mod timeout;

pub use breaker::{BreakerState, CircuitBreaker};
pub use clock::{Clock, ManualClock, SystemClock};
pub use limiter::RateLimiter;
pub use retry::{retry, RetryPolicy};
pub use timeout::{with_timeout, TimeoutError};
