//! Token-bucket rate limiter over an injectable [`Clock`].

use std::sync::Arc;

use parking_lot::Mutex;

use crate::clock::{Clock, SystemClock};

struct Bucket {
    tokens: f64,
    last_refill_nanos: u128,
}

/// A token-bucket rate limiter. Generic over the clock for deterministic tests.
pub struct RateLimiter<C: Clock = SystemClock> {
    capacity: f64,
    refill_per_sec: f64,
    bucket: Mutex<Bucket>,
    clock: Arc<C>,
}

impl RateLimiter<SystemClock> {
    /// Create a limiter with the given capacity and refill rate on the system clock.
    #[must_use]
    pub fn new(capacity: f64, refill_per_sec: f64) -> Self {
        RateLimiter::with_clock(capacity, refill_per_sec, Arc::new(SystemClock::new()))
    }
}

impl<C: Clock> RateLimiter<C> {
    /// Create a limiter on a specific clock.
    pub fn with_clock(capacity: f64, refill_per_sec: f64, clock: Arc<C>) -> Self {
        let now = clock.now_nanos();
        RateLimiter {
            capacity,
            refill_per_sec,
            bucket: Mutex::new(Bucket {
                tokens: capacity,
                last_refill_nanos: now,
            }),
            clock,
        }
    }

    /// Try to acquire a single token. Returns `true` if admitted.
    pub fn try_acquire(&self) -> bool {
        self.try_acquire_n(1.0)
    }

    /// Try to acquire `n` tokens. Returns `true` if admitted.
    pub fn try_acquire_n(&self, n: f64) -> bool {
        let now = self.clock.now_nanos();
        let mut bucket = self.bucket.lock();
        let elapsed_secs = (now.saturating_sub(bucket.last_refill_nanos)) as f64 / 1_000_000_000.0;
        bucket.tokens = (bucket.tokens + elapsed_secs * self.refill_per_sec).min(self.capacity);
        bucket.last_refill_nanos = now;
        if bucket.tokens >= n {
            bucket.tokens -= n;
            true
        } else {
            false
        }
    }
}
