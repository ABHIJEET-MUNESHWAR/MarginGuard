//! Advice value objects shared by every [`crate::RiskAdvisor`] backend.

use marginguard_types::{AccountHealth, MarketState, Position, Side};
use serde::{Deserialize, Serialize};

/// Qualitative liquidation-risk tier, ordered from safest to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Comfortable margin buffer.
    Safe,
    /// Buffer is shrinking; worth monitoring.
    Caution,
    /// Thin buffer; action recommended.
    Warning,
    /// At or beyond maintenance margin; liquidation imminent or active.
    Critical,
}

impl RiskLevel {
    /// A stable lower-case wire code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            RiskLevel::Safe => "safe",
            RiskLevel::Caution => "caution",
            RiskLevel::Warning => "warning",
            RiskLevel::Critical => "critical",
        }
    }
}

/// Which backend produced a piece of advice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdviceSource {
    /// Deterministic local heuristic.
    Heuristic,
    /// Large-language-model narration over the deterministic numbers.
    Llm,
}

impl AdviceSource {
    /// A stable wire code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            AdviceSource::Heuristic => "heuristic",
            AdviceSource::Llm => "llm",
        }
    }
}

/// A liquidation-risk assessment for a single position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskAdvice {
    /// Qualitative tier.
    pub risk_level: RiskLevel,
    /// Margin ratio (`equity / notional`) in basis points, when a position exists.
    pub margin_ratio_bps: Option<i64>,
    /// Approximate adverse price move to liquidation, in basis points of mark.
    ///
    /// Equals the cushion between the margin ratio and the maintenance ratio;
    /// non-positive means the position is already liquidatable.
    pub liquidation_distance_bps: Option<i64>,
    /// One-line recommended action.
    pub recommended_action: String,
    /// Human-readable narrative.
    pub summary: String,
    /// Confidence in `[0, 1]`.
    pub confidence: f64,
    /// Which backend produced this advice.
    pub source: AdviceSource,
}

/// Deterministic inputs an advisor reasons over. Built from engine read-model
/// value objects so advisors never touch the engine directly.
#[derive(Debug, Clone, PartialEq)]
pub struct AdviceContext {
    /// Position side.
    pub side: Side,
    /// Leverage multiplier.
    pub leverage: u32,
    /// Entry price in micro-USD.
    pub entry_price_micros: i128,
    /// Mark price in micro-USD.
    pub mark_price_micros: i128,
    /// Funding rate in basis points (positive = longs pay).
    pub funding_rate_bps: i64,
    /// Account equity in micro-USD.
    pub equity_micros: i128,
    /// Notional exposure in micro-USD.
    pub notional_micros: i128,
    /// Maintenance margin requirement in micro-USD.
    pub maintenance_margin_micros: i128,
    /// Margin ratio in basis points, when a position exists.
    pub margin_ratio_bps: Option<i64>,
    /// Whether the engine currently flags the account as liquidatable.
    pub liquidatable: bool,
}

impl AdviceContext {
    /// Assemble a context from a position and its health at a market state.
    #[must_use]
    pub fn from_parts(position: &Position, health: &AccountHealth, market: &MarketState) -> Self {
        AdviceContext {
            side: position.side,
            leverage: position.leverage.get(),
            entry_price_micros: position.entry_price.micros(),
            mark_price_micros: market.mark_price.micros(),
            funding_rate_bps: market.funding_rate_bps,
            equity_micros: health.equity.micros(),
            notional_micros: health.notional.micros(),
            maintenance_margin_micros: health.maintenance_margin.micros(),
            margin_ratio_bps: health.margin_ratio_bps,
            liquidatable: health.liquidatable,
        }
    }

    /// The maintenance margin ratio in basis points (`maintenance / notional`).
    #[must_use]
    pub fn maintenance_ratio_bps(&self) -> i64 {
        if self.notional_micros <= 0 {
            return 0;
        }
        i64::try_from(self.maintenance_margin_micros * 10_000 / self.notional_micros)
            .unwrap_or(i64::MAX)
    }

    /// Cushion in basis points between the margin ratio and the maintenance
    /// ratio. `None` when flat; non-positive means already liquidatable.
    #[must_use]
    pub fn liquidation_distance_bps(&self) -> Option<i64> {
        self.margin_ratio_bps
            .map(|mr| mr - self.maintenance_ratio_bps())
    }
}
