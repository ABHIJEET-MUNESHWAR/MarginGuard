//! Anti-corruption DTOs: GraphQL-facing types that never leak domain
//! invariants. Exact money/price/size values cross the boundary as strings of
//! micro-units (i128 does not fit a GraphQL `Int`), with an `f64` convenience
//! field for display only.

use async_graphql::SimpleObject;

use marginguard_ai::RiskAdvice;
use marginguard_core::{CommandOutcome, RiskEvent};
use marginguard_types::{AccountHealth, Liquidation, MarketState, Position, RiskStats, Usd};

/// A fixed-point quantity exposed as exact micros plus a display float.
#[derive(SimpleObject, Clone, Debug)]
pub struct Decimal {
    /// Exact value in micro-units (6 dp) as a base-10 string.
    pub micros: String,
    /// Convenience decimal (display only; never use for exact math).
    pub value: f64,
}

impl Decimal {
    /// Build from a raw micro value.
    #[must_use]
    pub fn from_micros(micros: i128) -> Self {
        Decimal {
            micros: micros.to_string(),
            #[allow(clippy::cast_precision_loss)]
            value: micros as f64 / 1_000_000.0,
        }
    }
}

impl From<Usd> for Decimal {
    fn from(u: Usd) -> Self {
        Decimal::from_micros(u.micros())
    }
}

/// An open position.
#[derive(SimpleObject, Clone, Debug)]
pub struct PositionDto {
    /// Owning account.
    pub account: String,
    /// Market symbol.
    pub symbol: String,
    /// `long` or `short`.
    pub side: String,
    /// `isolated` or `cross`.
    pub margin_mode: String,
    /// Position size.
    pub size: Decimal,
    /// Average entry price.
    pub entry_price: Decimal,
    /// Leverage multiplier.
    pub leverage: i32,
    /// Collateral posted.
    pub posted_margin: Decimal,
    /// Cumulative funding paid (positive) or received (negative).
    pub funding_paid: Decimal,
}

impl From<Position> for PositionDto {
    fn from(p: Position) -> Self {
        PositionDto {
            account: p.account.as_str().to_string(),
            symbol: p.symbol.as_str().to_string(),
            side: p.side.code().to_string(),
            margin_mode: p.margin_mode.code().to_string(),
            size: Decimal::from_micros(p.size.micros()),
            entry_price: Decimal::from_micros(p.entry_price.micros()),
            leverage: i32::try_from(p.leverage.get()).unwrap_or(i32::MAX),
            posted_margin: p.posted_margin.into(),
            funding_paid: p.funding_paid.into(),
        }
    }
}

/// An account's risk health for a market.
#[derive(SimpleObject, Clone, Debug)]
pub struct AccountHealthDto {
    /// Account.
    pub account: String,
    /// Equity = posted margin + unrealised PnL - funding owed.
    pub equity: Decimal,
    /// Notional exposure at the current mark.
    pub notional: Decimal,
    /// Maintenance margin required.
    pub maintenance_margin: Decimal,
    /// Margin ratio in basis points, or null when flat.
    pub margin_ratio_bps: Option<i64>,
    /// Whether the account currently breaches maintenance margin.
    pub liquidatable: bool,
}

impl From<AccountHealth> for AccountHealthDto {
    fn from(h: AccountHealth) -> Self {
        AccountHealthDto {
            account: h.account.as_str().to_string(),
            equity: h.equity.into(),
            notional: h.notional.into(),
            maintenance_margin: h.maintenance_margin.into(),
            margin_ratio_bps: h.margin_ratio_bps,
            liquidatable: h.liquidatable,
        }
    }
}

/// A market's price and funding state.
#[derive(SimpleObject, Clone, Debug)]
pub struct MarketStateDto {
    /// Market symbol.
    pub symbol: String,
    /// Mark price.
    pub mark_price: Decimal,
    /// Index price.
    pub index_price: Decimal,
    /// Funding rate in signed basis points.
    pub funding_rate_bps: i64,
}

impl From<MarketState> for MarketStateDto {
    fn from(m: MarketState) -> Self {
        MarketStateDto {
            symbol: m.symbol.as_str().to_string(),
            mark_price: Decimal::from_micros(m.mark_price.micros()),
            index_price: Decimal::from_micros(m.index_price.micros()),
            funding_rate_bps: m.funding_rate_bps,
        }
    }
}

/// The insurance fund balance.
#[derive(SimpleObject, Clone, Debug)]
pub struct InsuranceFundDto {
    /// Current balance.
    pub balance: Decimal,
}

/// Aggregate read-model statistics.
#[derive(SimpleObject, Clone, Debug)]
pub struct RiskStatsDto {
    /// Open positions.
    pub open_positions: u64,
    /// Liquidations performed.
    pub liquidations: u64,
    /// Funding settlements applied.
    pub funding_settlements: u64,
    /// Auto-deleverage events.
    pub adl_events: u64,
}

impl From<RiskStats> for RiskStatsDto {
    fn from(s: RiskStats) -> Self {
        RiskStatsDto {
            open_positions: s.open_positions,
            liquidations: s.liquidations,
            funding_settlements: s.funding_settlements,
            adl_events: s.adl_events,
        }
    }
}

/// A liquidation-risk assessment.
#[derive(SimpleObject, Clone, Debug)]
pub struct RiskAdviceDto {
    /// `safe` | `caution` | `warning` | `critical`.
    pub risk_level: String,
    /// Margin ratio in basis points, when a position exists.
    pub margin_ratio_bps: Option<i64>,
    /// Adverse price move to liquidation, in basis points of mark.
    pub liquidation_distance_bps: Option<i64>,
    /// One-line recommended action.
    pub recommended_action: String,
    /// Human-readable narrative.
    pub summary: String,
    /// Confidence in `[0, 1]`.
    pub confidence: f64,
    /// `heuristic` or `llm`.
    pub source: String,
}

impl From<RiskAdvice> for RiskAdviceDto {
    fn from(a: RiskAdvice) -> Self {
        RiskAdviceDto {
            risk_level: a.risk_level.code().to_string(),
            margin_ratio_bps: a.margin_ratio_bps,
            liquidation_distance_bps: a.liquidation_distance_bps,
            recommended_action: a.recommended_action,
            summary: a.summary,
            confidence: a.confidence,
            source: a.source.code().to_string(),
        }
    }
}

/// A liquidation record (carried inside a [`RiskEventDto`] payload too).
#[derive(SimpleObject, Clone, Debug)]
pub struct LiquidationDto {
    /// Liquidated account.
    pub account: String,
    /// Market.
    pub symbol: String,
    /// `maintenance_breach` or `bankruptcy`.
    pub reason: String,
    /// Notional closed.
    pub closed_notional: Decimal,
    /// Loss absorbed by the insurance fund.
    pub insurance_draw: Decimal,
    /// Loss socialised via auto-deleveraging.
    pub socialized_loss: Decimal,
}

impl From<Liquidation> for LiquidationDto {
    fn from(l: Liquidation) -> Self {
        LiquidationDto {
            account: l.account.as_str().to_string(),
            symbol: l.symbol.as_str().to_string(),
            reason: l.reason.code().to_string(),
            closed_notional: l.closed_notional.into(),
            insurance_draw: l.insurance_draw.into(),
            socialized_loss: l.socialized_loss.into(),
        }
    }
}

/// A risk event for subscriptions, carrying a stable `kind` and a lossless
/// JSON payload so the wire schema is stable as new event variants are added.
#[derive(SimpleObject, Clone, Debug)]
pub struct RiskEventDto {
    /// Stable event kind, e.g. `liquidated`.
    pub kind: String,
    /// The full event serialised as JSON.
    pub json: String,
}

impl From<&RiskEvent> for RiskEventDto {
    fn from(e: &RiskEvent) -> Self {
        RiskEventDto {
            kind: e.kind().to_string(),
            json: serde_json::to_string(e).unwrap_or_default(),
        }
    }
}

/// The result of applying a command.
#[derive(SimpleObject, Clone, Debug)]
pub struct OutcomeDto {
    /// Events emitted, in order.
    pub events: Vec<RiskEventDto>,
    /// Number of liquidation events in this outcome.
    pub liquidation_count: i32,
}

impl From<CommandOutcome> for OutcomeDto {
    fn from(o: CommandOutcome) -> Self {
        OutcomeDto {
            liquidation_count: i32::try_from(o.liquidation_count()).unwrap_or(i32::MAX),
            events: o.events.iter().map(RiskEventDto::from).collect(),
        }
    }
}
