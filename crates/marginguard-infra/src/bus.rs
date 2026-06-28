//! A broadcast event bus that is both an [`EventSink`] (write side) and a
//! [`RiskEventStream`] (read side) — the CQRS fan-out boundary.

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use marginguard_core::{EventSink, PortError, RiskEvent, RiskEventStream};

/// Fans engine events out to any number of live subscribers. Publishing is
/// best-effort: events emitted while no subscriber is attached are dropped,
/// which is the desired behaviour for a live notification stream.
#[derive(Clone)]
pub struct BroadcastEventSink {
    tx: broadcast::Sender<RiskEvent>,
}

impl BroadcastEventSink {
    /// Create a bus buffering up to `capacity` events per slow subscriber.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity.max(1));
        BroadcastEventSink { tx }
    }

    /// Number of currently attached subscribers.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for BroadcastEventSink {
    fn default() -> Self {
        BroadcastEventSink::new(1024)
    }
}

#[async_trait]
impl EventSink for BroadcastEventSink {
    async fn publish(&self, events: &[RiskEvent]) -> Result<(), PortError> {
        for event in events {
            // `send` errors only when there are no receivers; that is benign.
            let _ = self.tx.send(event.clone());
        }
        Ok(())
    }
}

impl RiskEventStream for BroadcastEventSink {
    fn subscribe(&self) -> BoxStream<'static, RiskEvent> {
        let rx = self.tx.subscribe();
        // Drop lagged/errored frames rather than terminating the stream.
        BroadcastStream::new(rx)
            .filter_map(|r| async move { r.ok() })
            .boxed()
    }
}
