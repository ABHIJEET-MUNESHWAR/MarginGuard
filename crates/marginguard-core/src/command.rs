//! Write-side commands accepted by the risk engine.

use marginguard_types::{AccountId, Leverage, MarginMode, Price, Side, Size, Symbol, Usd};

/// A command mutating engine state (the CQRS write side).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskCommand {
    /// Open a new position, posting collateral.
    OpenPosition {
        /// Owning account.
        account: AccountId,
        /// Market symbol.
        symbol: Symbol,
        /// Direction.
        side: Side,
        /// Margin mode.
        margin_mode: MarginMode,
        /// Size in contracts.
        size: Size,
        /// Entry price.
        entry_price: Price,
        /// Leverage.
        leverage: Leverage,
        /// Collateral posted.
        margin: Usd,
    },
    /// Close an existing position at the current mark price.
    ClosePosition {
        /// Owning account.
        account: AccountId,
        /// Market symbol.
        symbol: Symbol,
    },
    /// Update the mark/index price and funding rate for a market.
    UpdateMarket {
        /// Market symbol.
        symbol: Symbol,
        /// New mark price.
        mark_price: Price,
        /// New index price.
        index_price: Price,
        /// New funding rate in signed basis points.
        funding_rate_bps: i64,
    },
    /// Accrue one funding settlement across all positions in a market.
    AccrueFunding {
        /// Market symbol.
        symbol: Symbol,
    },
    /// Scan a market and liquidate every position breaching maintenance margin.
    LiquidateMarket {
        /// Market symbol.
        symbol: Symbol,
    },
}
