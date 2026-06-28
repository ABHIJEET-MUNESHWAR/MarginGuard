//! Mutation root: write-side resolvers translating GraphQL inputs into engine
//! commands. Inputs accept friendly whole-unit floats and are converted to
//! exact micro-units at the boundary; the engine math stays integer-exact.

use async_graphql::{Context, Enum, InputObject, Object, Result};

use marginguard_core::RiskCommand;
use marginguard_types::{AccountId, Leverage, MarginMode, Price, Side, Size, Symbol, Usd};

use crate::context::ApiContext;
use crate::dto::OutcomeDto;
use crate::error::to_err;

/// Position direction input.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum SideInput {
    /// Long.
    Long,
    /// Short.
    Short,
}

impl From<SideInput> for Side {
    fn from(s: SideInput) -> Self {
        match s {
            SideInput::Long => Side::Long,
            SideInput::Short => Side::Short,
        }
    }
}

/// Margin mode input.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MarginModeInput {
    /// Isolated margin.
    Isolated,
    /// Cross margin.
    Cross,
}

impl From<MarginModeInput> for MarginMode {
    fn from(m: MarginModeInput) -> Self {
        match m {
            MarginModeInput::Isolated => MarginMode::Isolated,
            MarginModeInput::Cross => MarginMode::Cross,
        }
    }
}

/// Inputs to open a position. Prices/sizes are whole units (e.g. `100.5`).
#[derive(InputObject)]
pub struct OpenPositionInput {
    /// Account id.
    pub account: String,
    /// Market symbol.
    pub symbol: String,
    /// Long or short.
    pub side: SideInput,
    /// Isolated or cross.
    pub margin_mode: MarginModeInput,
    /// Position size in contracts.
    pub size: f64,
    /// Entry price in USD.
    pub entry_price: f64,
    /// Leverage multiplier (1..=100).
    pub leverage: i32,
    /// Collateral posted in USD.
    pub margin: f64,
}

#[allow(clippy::cast_possible_truncation)]
fn to_micros(whole: f64) -> i128 {
    (whole * 1_000_000.0).round() as i128
}

/// The GraphQL mutation root.
#[derive(Default)]
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Open a new position.
    async fn open_position(
        &self,
        ctx: &Context<'_>,
        input: OpenPositionInput,
    ) -> Result<OutcomeDto> {
        let cx = ctx.data::<ApiContext>()?;
        let leverage =
            u32::try_from(input.leverage).map_err(|_| to_err("leverage out of range"))?;
        let command = RiskCommand::OpenPosition {
            account: AccountId::new(input.account).map_err(to_err)?,
            symbol: Symbol::new(input.symbol).map_err(to_err)?,
            side: input.side.into(),
            margin_mode: input.margin_mode.into(),
            size: Size::from_micros(to_micros(input.size)).map_err(to_err)?,
            entry_price: Price::from_micros(to_micros(input.entry_price)).map_err(to_err)?,
            leverage: Leverage::new(leverage).map_err(to_err)?,
            margin: Usd::from_micros(to_micros(input.margin)),
        };
        Ok(cx.engine.apply(command).await.map_err(to_err)?.into())
    }

    /// Close a position at the current mark, realising PnL.
    async fn close_position(
        &self,
        ctx: &Context<'_>,
        account: String,
        symbol: String,
    ) -> Result<OutcomeDto> {
        let cx = ctx.data::<ApiContext>()?;
        let command = RiskCommand::ClosePosition {
            account: AccountId::new(account).map_err(to_err)?,
            symbol: Symbol::new(symbol).map_err(to_err)?,
        };
        Ok(cx.engine.apply(command).await.map_err(to_err)?.into())
    }

    /// Update a market's mark/index price and funding rate.
    async fn update_market(
        &self,
        ctx: &Context<'_>,
        symbol: String,
        mark_price: f64,
        index_price: f64,
        funding_rate_bps: i64,
    ) -> Result<OutcomeDto> {
        let cx = ctx.data::<ApiContext>()?;
        let command = RiskCommand::UpdateMarket {
            symbol: Symbol::new(symbol).map_err(to_err)?,
            mark_price: Price::from_micros(to_micros(mark_price)).map_err(to_err)?,
            index_price: Price::from_micros(to_micros(index_price)).map_err(to_err)?,
            funding_rate_bps,
        };
        Ok(cx.engine.apply(command).await.map_err(to_err)?.into())
    }

    /// Accrue one funding interval across all positions in a market.
    async fn accrue_funding(&self, ctx: &Context<'_>, symbol: String) -> Result<OutcomeDto> {
        let cx = ctx.data::<ApiContext>()?;
        let command = RiskCommand::AccrueFunding {
            symbol: Symbol::new(symbol).map_err(to_err)?,
        };
        Ok(cx.engine.apply(command).await.map_err(to_err)?.into())
    }

    /// Run the liquidation waterfall across all underwater positions.
    async fn liquidate_market(&self, ctx: &Context<'_>, symbol: String) -> Result<OutcomeDto> {
        let cx = ctx.data::<ApiContext>()?;
        let command = RiskCommand::LiquidateMarket {
            symbol: Symbol::new(symbol).map_err(to_err)?,
        };
        Ok(cx.engine.apply(command).await.map_err(to_err)?.into())
    }
}
