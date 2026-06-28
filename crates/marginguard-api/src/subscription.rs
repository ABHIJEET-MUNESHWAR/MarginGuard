//! Subscription root: live event streams backed by the broadcast bus.

use async_graphql::{Context, Result, Subscription};
use futures::{Stream, StreamExt};

use crate::context::ApiContext;
use crate::dto::RiskEventDto;

/// The GraphQL subscription root.
#[derive(Default)]
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Every engine event as it happens.
    async fn risk_events(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl Stream<Item = RiskEventDto> + 'static> {
        let cx = ctx.data::<ApiContext>()?;
        let stream = cx.events.subscribe();
        Ok(stream.map(|e| RiskEventDto::from(&e)))
    }

    /// Only liquidation and auto-deleverage events — a focused alert feed.
    async fn liquidation_alerts(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl Stream<Item = RiskEventDto> + 'static> {
        let cx = ctx.data::<ApiContext>()?;
        let stream = cx.events.subscribe();
        Ok(stream.filter_map(|e| async move {
            matches!(e.kind(), "liquidated" | "auto_deleveraged").then(|| RiskEventDto::from(&e))
        }))
    }
}
