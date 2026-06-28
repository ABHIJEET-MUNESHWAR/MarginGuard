//! Read-side events emitted by the risk engine (the CQRS read side / event log).

use serde::{Deserialize, Serialize};

use marginguard_types::{AccountId, Liquidation, Side, Symbol, Usd};

/// An event describing a state change in the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RiskEvent {
    /// A position was opened.
    PositionOpened {
        /// Account.
        account: AccountId,
        /// Market.
        symbol: Symbol,
        /// Direction.
        side: Side,
        /// Notional opened.
        notional: Usd,
    },
    /// A position was closed (voluntarily), realising PnL.
    PositionClosed {
        /// Account.
        account: AccountId,
        /// Market.
        symbol: Symbol,
        /// Realised PnL.
        realized_pnl: Usd,
    },
    /// A market's mark/index/funding state changed.
    MarketUpdated {
        /// Market.
        symbol: Symbol,
        /// New mark price in micro-USD.
        mark_price: i128,
        /// Funding rate in signed basis points.
        funding_rate_bps: i64,
    },
    /// A funding settlement was applied to a position.
    FundingSettled {
        /// Account.
        account: AccountId,
        /// Market.
        symbol: Symbol,
        /// Signed funding amount (positive = paid by this position).
        amount: Usd,
    },
    /// A position was liquidated.
    Liquidated(Liquidation),
    /// Loss was socialised across counterparties via auto-deleveraging.
    AutoDeleveraged {
        /// Market.
        symbol: Symbol,
        /// Total loss socialised.
        socialized_loss: Usd,
    },
}

impl RiskEvent {
    /// A stable wire kind.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            RiskEvent::PositionOpened { .. } => "position_opened",
            RiskEvent::PositionClosed { .. } => "position_closed",
            RiskEvent::MarketUpdated { .. } => "market_updated",
            RiskEvent::FundingSettled { .. } => "funding_settled",
            RiskEvent::Liquidated(_) => "liquidated",
            RiskEvent::AutoDeleveraged { .. } => "auto_deleveraged",
        }
    }
}
