//! Positions and the per-market state used to value them.

use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, Leverage, Price, Size, Symbol};
use crate::money::{scaled_mul, Usd};

/// Direction of a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// Long: profits when the mark price rises.
    Long,
    /// Short: profits when the mark price falls.
    Short,
}

impl Side {
    /// The opposing side.
    #[must_use]
    pub const fn opposite(self) -> Side {
        match self {
            Side::Long => Side::Short,
            Side::Short => Side::Long,
        }
    }

    /// A stable wire code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Side::Long => "long",
            Side::Short => "short",
        }
    }

    /// Sign multiplier: `+1` for long, `-1` for short.
    #[must_use]
    pub const fn sign(self) -> i128 {
        match self {
            Side::Long => 1,
            Side::Short => -1,
        }
    }
}

/// Margin mode governing how collateral backs a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarginMode {
    /// Isolated: only the position's own posted margin is at risk.
    Isolated,
    /// Cross: the account's whole free balance backs the position.
    Cross,
}

impl MarginMode {
    /// A stable wire code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            MarginMode::Isolated => "isolated",
            MarginMode::Cross => "cross",
        }
    }
}

/// An open perpetual-futures position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Owning account.
    pub account: AccountId,
    /// Market symbol.
    pub symbol: Symbol,
    /// Long or short.
    pub side: Side,
    /// Margin mode.
    pub margin_mode: MarginMode,
    /// Position size (always positive; direction is `side`).
    pub size: Size,
    /// Average entry price.
    pub entry_price: Price,
    /// Chosen leverage.
    pub leverage: Leverage,
    /// Collateral posted to this position (isolated) or allocated (cross).
    pub posted_margin: Usd,
    /// Cumulative realised funding paid (positive) or received (negative).
    pub funding_paid: Usd,
}

impl Position {
    /// Notional value of the position at the given mark price (always >= 0).
    #[must_use]
    pub fn notional(&self, mark: Price) -> Usd {
        Usd::from_micros(scaled_mul(mark.micros(), self.size.micros()))
    }

    /// Unrealised PnL at the given mark price.
    ///
    /// Long: `(mark - entry) * size`. Short: `(entry - mark) * size`.
    #[must_use]
    pub fn unrealized_pnl(&self, mark: Price) -> Usd {
        let diff = (mark.micros() - self.entry_price.micros()) * self.side.sign();
        Usd::from_micros(scaled_mul(diff, self.size.micros()))
    }
}
