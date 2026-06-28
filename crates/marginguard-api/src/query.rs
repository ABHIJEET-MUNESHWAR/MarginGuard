//! Query root: read-model resolvers over the engine and advisor.

use async_graphql::{Context, Object, Result};

use marginguard_ai::AdviceContext;
use marginguard_types::Symbol;

use crate::context::ApiContext;
use crate::dto::{
    AccountHealthDto, InsuranceFundDto, MarketStateDto, PositionDto, RiskAdviceDto, RiskStatsDto,
};
use crate::error::to_err;

/// The GraphQL query root.
#[derive(Default)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// The running API version.
    async fn api_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// A single position by account and market, if open.
    async fn position(
        &self,
        ctx: &Context<'_>,
        account: String,
        symbol: String,
    ) -> Result<Option<PositionDto>> {
        let cx = ctx.data::<ApiContext>()?;
        let sym = Symbol::new(symbol).map_err(to_err)?;
        let pos = cx.engine.position(&account, &sym).await.map_err(to_err)?;
        Ok(pos.map(PositionDto::from))
    }

    /// All positions currently open in a market.
    async fn positions(&self, ctx: &Context<'_>, symbol: String) -> Result<Vec<PositionDto>> {
        let cx = ctx.data::<ApiContext>()?;
        let sym = Symbol::new(symbol).map_err(to_err)?;
        let positions = cx.engine.positions_in(&sym).await.map_err(to_err)?;
        Ok(positions.into_iter().map(PositionDto::from).collect())
    }

    /// The risk health of an account's position in a market.
    async fn account_health(
        &self,
        ctx: &Context<'_>,
        account: String,
        symbol: String,
    ) -> Result<AccountHealthDto> {
        let cx = ctx.data::<ApiContext>()?;
        let sym = Symbol::new(symbol).map_err(to_err)?;
        let health = cx
            .engine
            .account_health(&account, &sym)
            .await
            .map_err(to_err)?;
        Ok(health.into())
    }

    /// The current state of a market, if known.
    async fn market_state(
        &self,
        ctx: &Context<'_>,
        symbol: String,
    ) -> Result<Option<MarketStateDto>> {
        let cx = ctx.data::<ApiContext>()?;
        let sym = Symbol::new(symbol).map_err(to_err)?;
        Ok(cx.engine.market(&sym).map(MarketStateDto::from))
    }

    /// The insurance fund balance.
    async fn insurance_fund(&self, ctx: &Context<'_>) -> Result<InsuranceFundDto> {
        let cx = ctx.data::<ApiContext>()?;
        Ok(InsuranceFundDto {
            balance: cx.engine.insurance_balance().into(),
        })
    }

    /// Aggregate engine statistics.
    async fn risk_stats(&self, ctx: &Context<'_>) -> Result<RiskStatsDto> {
        let cx = ctx.data::<ApiContext>()?;
        Ok(cx.engine.stats().into())
    }

    /// A liquidation-risk assessment for a position, via the configured advisor
    /// (deterministic heuristic, optionally narrated by an LLM).
    async fn risk_advice(
        &self,
        ctx: &Context<'_>,
        account: String,
        symbol: String,
    ) -> Result<RiskAdviceDto> {
        let cx = ctx.data::<ApiContext>()?;
        let sym = Symbol::new(symbol).map_err(to_err)?;
        let pos = cx
            .engine
            .position(&account, &sym)
            .await
            .map_err(to_err)?
            .ok_or_else(|| to_err("position not found"))?;
        let health = cx
            .engine
            .account_health(&account, &sym)
            .await
            .map_err(to_err)?;
        let market = cx
            .engine
            .market(&sym)
            .ok_or_else(|| to_err("market not found"))?;
        let advice_ctx = AdviceContext::from_parts(&pos, &health, &market);
        Ok(cx.advisor.assess(&advice_ctx).await.into())
    }
}
