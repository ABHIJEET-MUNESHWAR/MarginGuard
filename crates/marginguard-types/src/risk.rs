//! Outcome value objects: account health, funding payments, and liquidations.

use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, Symbol};
use crate::money::Usd;

/// A snapshot of an account's risk health for one market (or aggregated).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountHealth {
    /// Account.
    pub account: AccountId,
    /// Equity = posted margin + unrealised PnL - funding owed.
    pub equity: Usd,
    /// Notional exposure at the current mark.
    pub notional: Usd,
    /// Maintenance margin required.
    pub maintenance_margin: Usd,
    /// Margin ratio in basis points (`equity / notional`), or `None` when flat.
    pub margin_ratio_bps: Option<i64>,
    /// Whether the account currently breaches maintenance margin.
    pub liquidatable: bool,
}

/// The reason a liquidation occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiquidationReason {
    /// Maintenance margin breached at the prevailing mark price.
    MaintenanceBreach,
    /// Bankrupt: equity fell below zero (insurance fund / ADL engaged).
    Bankruptcy,
}

impl LiquidationReason {
    /// A stable wire code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            LiquidationReason::MaintenanceBreach => "maintenance_breach",
            LiquidationReason::Bankruptcy => "bankruptcy",
        }
    }
}

/// The record of a completed liquidation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Liquidation {
    /// Liquidated account.
    pub account: AccountId,
    /// Market.
    pub symbol: Symbol,
    /// Why it was liquidated.
    pub reason: LiquidationReason,
    /// Notional closed.
    pub closed_notional: Usd,
    /// Loss absorbed by the insurance fund (zero if the position was solvent).
    pub insurance_draw: Usd,
    /// Loss socialised via auto-deleveraging after the fund was exhausted.
    pub socialized_loss: Usd,
}

/// Aggregate statistics for the risk engine read model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskStats {
    /// Number of open positions.
    pub open_positions: u64,
    /// Number of liquidations performed.
    pub liquidations: u64,
    /// Number of funding settlements applied.
    pub funding_settlements: u64,
    /// Number of auto-deleverage events.
    pub adl_events: u64,
}
