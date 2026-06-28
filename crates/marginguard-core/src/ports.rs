//! Outbound ports (hexagonal boundaries) the engine depends on.

use async_trait::async_trait;
use futures::stream::BoxStream;

use marginguard_types::{Position, Symbol};

use crate::error::PortError;
use crate::event::RiskEvent;

/// Persists and retrieves positions.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait PositionStore: Send + Sync {
    /// Insert or replace a position.
    async fn upsert(&self, position: Position) -> Result<(), PortError>;

    /// Fetch a single position by account and market.
    async fn get(&self, account: &str, symbol: &Symbol) -> Result<Option<Position>, PortError>;

    /// Remove a position, returning it if present.
    async fn remove(&self, account: &str, symbol: &Symbol) -> Result<Option<Position>, PortError>;

    /// All positions in a market.
    async fn by_market(&self, symbol: &Symbol) -> Result<Vec<Position>, PortError>;

    /// Total number of open positions.
    async fn count(&self) -> Result<u64, PortError>;
}

/// Publishes engine events to interested consumers.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait EventSink: Send + Sync {
    /// Publish a batch of events.
    async fn publish(&self, events: &[RiskEvent]) -> Result<(), PortError>;
}

/// A subscribable stream of engine events (for GraphQL subscriptions).
pub trait RiskEventStream: Send + Sync {
    /// Subscribe to all future events.
    fn subscribe(&self) -> BoxStream<'static, RiskEvent>;
}
