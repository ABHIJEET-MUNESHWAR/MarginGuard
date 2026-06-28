//! Per-market state (mark/index price, funding) and risk-tier parameters.

use serde::{Deserialize, Serialize};

use crate::ids::{Price, Symbol};

/// Live state for a single perpetual market.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketState {
    /// Market symbol.
    pub symbol: Symbol,
    /// Current mark price (used for PnL and liquidation).
    pub mark_price: Price,
    /// Current index/oracle price (the funding anchor).
    pub index_price: Price,
    /// Current funding rate in signed basis points per funding interval.
    /// Positive means longs pay shorts.
    pub funding_rate_bps: i64,
}

impl MarketState {
    /// Construct a market state with equal mark/index and zero funding.
    #[must_use]
    pub fn flat(symbol: Symbol, price: Price) -> Self {
        MarketState {
            symbol,
            mark_price: price,
            index_price: price,
            funding_rate_bps: 0,
        }
    }
}

/// Risk parameters governing margin requirements for a market.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskParams {
    /// Initial-margin requirement in basis points of notional.
    pub initial_margin_bps: i64,
    /// Maintenance-margin requirement in basis points of notional.
    pub maintenance_margin_bps: i64,
    /// Liquidation penalty in basis points of notional, paid to the
    /// insurance fund on liquidation.
    pub liquidation_fee_bps: i64,
}

impl RiskParams {
    /// A sensible default tier: 5% initial, 2.5% maintenance, 1% fee.
    #[must_use]
    pub const fn standard() -> Self {
        RiskParams {
            initial_margin_bps: 500,
            maintenance_margin_bps: 250,
            liquidation_fee_bps: 100,
        }
    }
}

impl Default for RiskParams {
    fn default() -> Self {
        RiskParams::standard()
    }
}
