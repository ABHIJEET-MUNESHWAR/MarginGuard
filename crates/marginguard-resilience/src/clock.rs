//! Injectable monotonic clock so time-dependent logic is deterministic in tests.

use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;

/// A source of monotonic time in nanoseconds.
pub trait Clock: Send + Sync + 'static {
    /// Nanoseconds since an arbitrary fixed epoch.
    fn now_nanos(&self) -> u128;
}

/// A real clock backed by [`std::time::Instant`].
#[derive(Clone)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// Create a system clock anchored at the current instant.
    #[must_use]
    pub fn new() -> Self {
        SystemClock {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        SystemClock::new()
    }
}

impl Clock for SystemClock {
    fn now_nanos(&self) -> u128 {
        self.origin.elapsed().as_nanos()
    }
}

/// A manually advanced clock for deterministic tests.
#[derive(Clone, Default)]
pub struct ManualClock {
    nanos: Arc<Mutex<u128>>,
}

impl ManualClock {
    /// Create a manual clock starting at zero.
    #[must_use]
    pub fn new() -> Self {
        ManualClock {
            nanos: Arc::new(Mutex::new(0)),
        }
    }

    /// Advance the clock by `nanos`.
    pub fn advance(&self, nanos: u128) {
        *self.nanos.lock() += nanos;
    }
}

impl Clock for ManualClock {
    fn now_nanos(&self) -> u128 {
        *self.nanos.lock()
    }
}
