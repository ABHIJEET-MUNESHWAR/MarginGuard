//! # marginguard-infra
//!
//! Concrete adapters that satisfy the core's outbound ports:
//!
//! * [`MemoryPositionStore`] — fast in-memory store with a by-market index.
//! * [`BroadcastEventSink`] — a CQRS fan-out that is both an `EventSink` and a
//!   `RiskEventStream`.
//! * [`SimMarketOracle`] — a deterministic mark-price walk for simulations.
//! * `PgPositionStore` (feature `postgres`) — a hash-partitioned Postgres store.

#![forbid(unsafe_code)]

pub mod bus;
pub mod oracle;
pub mod store;

#[cfg(feature = "postgres")]
pub mod pg;

pub use bus::BroadcastEventSink;
pub use oracle::SimMarketOracle;
pub use store::MemoryPositionStore;

#[cfg(feature = "postgres")]
pub use pg::PgPositionStore;
