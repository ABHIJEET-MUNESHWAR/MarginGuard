//! Advisor trait and the always-available deterministic heuristic backend.

use async_trait::async_trait;

use crate::advice::{AdviceContext, AdviceSource, RiskAdvice, RiskLevel};

/// A liquidation-risk advisor. Implementations must be cheap to clone behind an
/// `Arc` and safe to call from many tasks concurrently.
#[async_trait]
pub trait RiskAdvisor: Send + Sync {
    /// Produce advice for the given context. Must never panic and should never
    /// fail: networked backends degrade to a deterministic result instead.
    async fn assess(&self, ctx: &AdviceContext) -> RiskAdvice;
}

/// Buffer (bps) at or below which a position is treated as critical.
const CRITICAL_BPS: i64 = 0;
/// Buffer (bps) at or below which a position is a warning.
const WARNING_BPS: i64 = 200;
/// Buffer (bps) at or below which a position warrants caution.
const CAUTION_BPS: i64 = 500;

/// A deterministic, dependency-free advisor derived purely from the margin
/// cushion. Always available and used as the fallback for networked backends.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeuristicAdvisor;

impl HeuristicAdvisor {
    /// Construct a heuristic advisor.
    #[must_use]
    pub const fn new() -> Self {
        HeuristicAdvisor
    }

    /// Pure scoring routine, exposed for reuse by networked backends that want
    /// the same deterministic numbers underneath their narration.
    #[must_use]
    pub fn score(&self, ctx: &AdviceContext) -> RiskAdvice {
        let distance = ctx.liquidation_distance_bps();
        let level = classify(ctx.liquidatable, distance);
        let (recommended_action, confidence) = match level {
            RiskLevel::Critical => (
                "Add margin or reduce size immediately to avoid liquidation.",
                0.90,
            ),
            RiskLevel::Warning => (
                "Margin buffer is thin; add margin or trim exposure soon.",
                0.75,
            ),
            RiskLevel::Caution => ("Monitor closely; the margin buffer is moderate.", 0.60),
            RiskLevel::Safe => (
                "No action needed; the position is well collateralised.",
                0.55,
            ),
        };
        let summary = build_summary(level, ctx, distance);
        RiskAdvice {
            risk_level: level,
            margin_ratio_bps: ctx.margin_ratio_bps,
            liquidation_distance_bps: distance,
            recommended_action: recommended_action.to_string(),
            summary,
            confidence,
            source: AdviceSource::Heuristic,
        }
    }
}

#[async_trait]
impl RiskAdvisor for HeuristicAdvisor {
    async fn assess(&self, ctx: &AdviceContext) -> RiskAdvice {
        self.score(ctx)
    }
}

fn classify(liquidatable: bool, distance: Option<i64>) -> RiskLevel {
    if liquidatable {
        return RiskLevel::Critical;
    }
    match distance {
        Some(d) if d <= CRITICAL_BPS => RiskLevel::Critical,
        Some(d) if d <= WARNING_BPS => RiskLevel::Warning,
        Some(d) if d <= CAUTION_BPS => RiskLevel::Caution,
        Some(_) => RiskLevel::Safe,
        None => RiskLevel::Safe,
    }
}

fn build_summary(level: RiskLevel, ctx: &AdviceContext, distance: Option<i64>) -> String {
    let side = ctx.side.code();
    match (level, distance) {
        (RiskLevel::Safe, None) => "Account is flat; no liquidation risk.".to_string(),
        (_, Some(d)) => format!(
            "{side} position at {lev}x is {tier}: margin buffer is {d} bps from \
             liquidation (funding {fund} bps).",
            lev = ctx.leverage,
            tier = level.code(),
            fund = ctx.funding_rate_bps,
        ),
        (_, None) => format!("{side} position is {tier}.", tier = level.code()),
    }
}
