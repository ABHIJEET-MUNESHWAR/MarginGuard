//! A minimal circuit breaker over an injectable [`Clock`].

use std::sync::Arc;

use parking_lot::Mutex;

use crate::clock::{Clock, SystemClock};

/// The breaker's externally observable state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    /// Requests pass through; failures are counted.
    Closed,
    /// Requests are rejected until the cooldown elapses.
    Open,
    /// A single trial request is allowed to test recovery.
    HalfOpen,
}

impl BreakerState {
    /// A stable wire name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            BreakerState::Closed => "closed",
            BreakerState::Open => "open",
            BreakerState::HalfOpen => "half_open",
        }
    }
}

struct Inner {
    consecutive_failures: u32,
    opened_at_nanos: Option<u128>,
    half_open: bool,
}

/// A circuit breaker that opens after `failure_threshold` consecutive failures
/// and transitions to half-open after a cooldown.
pub struct CircuitBreaker<C: Clock = SystemClock> {
    failure_threshold: u32,
    cooldown_nanos: u128,
    inner: Mutex<Inner>,
    clock: Arc<C>,
}

impl CircuitBreaker<SystemClock> {
    /// Create a breaker on the system clock.
    #[must_use]
    pub fn new(failure_threshold: u32, cooldown: std::time::Duration) -> Self {
        CircuitBreaker::with_clock(failure_threshold, cooldown, Arc::new(SystemClock::new()))
    }
}

impl<C: Clock> CircuitBreaker<C> {
    /// Create a breaker on a specific clock.
    pub fn with_clock(
        failure_threshold: u32,
        cooldown: std::time::Duration,
        clock: Arc<C>,
    ) -> Self {
        CircuitBreaker {
            failure_threshold,
            cooldown_nanos: cooldown.as_nanos(),
            inner: Mutex::new(Inner {
                consecutive_failures: 0,
                opened_at_nanos: None,
                half_open: false,
            }),
            clock,
        }
    }

    /// Current breaker state.
    pub fn state(&self) -> BreakerState {
        let mut inner = self.inner.lock();
        self.refresh(&mut inner);
        if inner.opened_at_nanos.is_some() {
            if inner.half_open {
                BreakerState::HalfOpen
            } else {
                BreakerState::Open
            }
        } else {
            BreakerState::Closed
        }
    }

    /// Whether a request may proceed right now.
    pub fn allow(&self) -> bool {
        !matches!(self.state(), BreakerState::Open)
    }

    /// Record a successful call, closing the breaker.
    pub fn on_success(&self) {
        let mut inner = self.inner.lock();
        inner.consecutive_failures = 0;
        inner.opened_at_nanos = None;
        inner.half_open = false;
    }

    /// Record a failed call, opening the breaker at the threshold.
    pub fn on_failure(&self) {
        let mut inner = self.inner.lock();
        inner.consecutive_failures += 1;
        inner.half_open = false;
        if inner.consecutive_failures >= self.failure_threshold {
            inner.opened_at_nanos = Some(self.clock.now_nanos());
        }
    }

    fn refresh(&self, inner: &mut Inner) {
        if let Some(opened) = inner.opened_at_nanos {
            let elapsed = self.clock.now_nanos().saturating_sub(opened);
            if elapsed >= self.cooldown_nanos {
                inner.half_open = true;
            }
        }
    }
}
