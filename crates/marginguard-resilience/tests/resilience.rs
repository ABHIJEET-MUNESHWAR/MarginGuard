//! Tests for the resilience primitives using a `ManualClock` and paused time.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use marginguard_resilience::{
    retry, with_timeout, BreakerState, CircuitBreaker, ManualClock, RateLimiter, RetryPolicy,
    TimeoutError,
};

#[tokio::test(start_paused = true)]
async fn with_timeout_succeeds_fast() {
    let result = with_timeout(Duration::from_secs(1), async { 7 }).await;
    assert_eq!(result, Ok(7));
}

#[tokio::test(start_paused = true)]
async fn with_timeout_fails_slow() {
    let result: Result<(), TimeoutError> = with_timeout(Duration::from_millis(10), async {
        tokio::time::sleep(Duration::from_secs(60)).await;
    })
    .await;
    assert!(result.is_err());
}

#[test]
fn rate_limiter_admits_up_to_capacity() {
    let clock = Arc::new(ManualClock::new());
    let limiter = RateLimiter::with_clock(3.0, 1.0, clock.clone());
    assert!(limiter.try_acquire());
    assert!(limiter.try_acquire());
    assert!(limiter.try_acquire());
    assert!(!limiter.try_acquire());
}

#[test]
fn rate_limiter_refills_over_time() {
    let clock = Arc::new(ManualClock::new());
    let limiter = RateLimiter::with_clock(1.0, 10.0, clock.clone());
    assert!(limiter.try_acquire());
    assert!(!limiter.try_acquire());
    clock.advance(200_000_000); // 0.2s -> 2 tokens at 10/s
    assert!(limiter.try_acquire());
}

#[test]
fn breaker_opens_after_threshold_and_recovers() {
    let clock = Arc::new(ManualClock::new());
    let breaker = CircuitBreaker::with_clock(2, Duration::from_secs(5), clock.clone());
    assert_eq!(breaker.state(), BreakerState::Closed);
    breaker.on_failure();
    assert!(breaker.allow());
    breaker.on_failure();
    assert_eq!(breaker.state(), BreakerState::Open);
    assert!(!breaker.allow());
    clock.advance(5_000_000_000);
    assert_eq!(breaker.state(), BreakerState::HalfOpen);
    breaker.on_success();
    assert_eq!(breaker.state(), BreakerState::Closed);
}

#[tokio::test(start_paused = true)]
async fn retry_eventually_succeeds() {
    let calls = Arc::new(AtomicU32::new(0));
    let calls2 = calls.clone();
    let policy = RetryPolicy::new(5, Duration::from_millis(10), Duration::from_secs(1));
    let result: Result<u32, &str> = retry(
        policy,
        |_| true,
        move || {
            let calls = calls2.clone();
            async move {
                let n = calls.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err("transient")
                } else {
                    Ok(n)
                }
            }
        },
    )
    .await;
    assert_eq!(result, Ok(2));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test(start_paused = true)]
async fn retry_stops_on_non_retryable() {
    let calls = Arc::new(AtomicU32::new(0));
    let calls2 = calls.clone();
    let policy = RetryPolicy::default();
    let result: Result<(), &str> = retry(
        policy,
        |_| false,
        move || {
            let calls = calls2.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err("fatal")
            }
        },
    )
    .await;
    assert_eq!(result, Err("fatal"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn backoff_is_bounded_and_deterministic() {
    let policy = RetryPolicy::new(10, Duration::from_millis(100), Duration::from_secs(2));
    let a = policy.backoff(3);
    let b = policy.backoff(3);
    assert_eq!(a, b);
    assert!(a <= Duration::from_secs(2));
}

#[test]
fn breaker_state_names_are_stable() {
    assert_eq!(BreakerState::Closed.name(), "closed");
    assert_eq!(BreakerState::Open.name(), "open");
    assert_eq!(BreakerState::HalfOpen.name(), "half_open");
}
