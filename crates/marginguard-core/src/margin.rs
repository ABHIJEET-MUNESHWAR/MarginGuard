//! Pure margin mathematics.
//!
//! Every function here is deterministic and side-effect-free, operating on
//! exact fixed-point [`Usd`] values. This is the auditable core that the
//! solvency property tests exercise.

use marginguard_types::{AccountHealth, MarginMode, Position, Price, RiskParams, Side, Usd};

/// Equity backing a single position at the given mark price.
///
/// `equity = posted_margin + unrealised_pnl - funding_owed`.
#[must_use]
pub fn position_equity(pos: &Position, mark: Price) -> Usd {
    pos.posted_margin
        .saturating_add(pos.unrealized_pnl(mark))
        .saturating_sub(pos.funding_paid)
}

/// Maintenance margin required for a position at the given mark price.
#[must_use]
pub fn maintenance_margin(pos: &Position, mark: Price, params: &RiskParams) -> Usd {
    pos.notional(mark).mul_bps(params.maintenance_margin_bps)
}

/// Initial margin required for a position at the given mark price.
#[must_use]
pub fn initial_margin(pos: &Position, mark: Price, params: &RiskParams) -> Usd {
    pos.notional(mark).mul_bps(params.initial_margin_bps)
}

/// Margin ratio in basis points (`equity / notional * 10_000`), or `None`
/// when the position has no notional value.
#[must_use]
pub fn margin_ratio_bps(equity: Usd, notional: Usd) -> Option<i64> {
    if notional.micros() == 0 {
        return None;
    }
    let ratio = equity.micros().saturating_mul(10_000) / notional.micros();
    Some(i64::try_from(ratio).unwrap_or(i64::MAX))
}

/// Whether a position breaches maintenance margin at the given mark price.
#[must_use]
pub fn is_liquidatable(pos: &Position, mark: Price, params: &RiskParams) -> bool {
    position_equity(pos, mark) < maintenance_margin(pos, mark, params)
}

/// The mark price at which a position would hit maintenance margin.
///
/// Derived from `equity(P) = maintenance(P)`. Returns `None` if the maintenance
/// rate is so high the position is liquidatable at any price.
#[must_use]
pub fn liquidation_price(pos: &Position, params: &RiskParams) -> Option<Price> {
    // Let s = size (contracts), e = entry, m = mark, sign = +1 long / -1 short,
    // margin0 = posted_margin - funding_paid (constant in m).
    // equity(m)      = margin0 + sign * (m - e) * s
    // maintenance(m) = mm_bps/10_000 * m * s
    // Solve equity = maintenance for m:
    //   margin0 - sign*e*s = m*s*(mm_bps/10_000 - sign)
    let size = pos.size.micros() as f64 / 1e6;
    let entry = pos.entry_price.micros() as f64 / 1e6;
    let margin0 = (pos.posted_margin.micros() - pos.funding_paid.micros()) as f64 / 1e6;
    let sign = pos.side.sign() as f64;
    let mm = params.maintenance_margin_bps as f64 / 10_000.0;

    let denom = size * (mm - sign);
    if denom == 0.0 {
        return None;
    }
    let numer = margin0 - sign * entry * size;
    let price = numer / denom;
    if price <= 0.0 {
        return None;
    }
    Price::from_micros((price * 1e6) as i128).ok()
}

/// Compute an [`AccountHealth`] view for a single position.
#[must_use]
pub fn account_health(pos: &Position, mark: Price, params: &RiskParams) -> AccountHealth {
    let equity = position_equity(pos, mark);
    let notional = pos.notional(mark);
    let maintenance = maintenance_margin(pos, mark, params);
    AccountHealth {
        account: pos.account.clone(),
        equity,
        notional,
        maintenance_margin: maintenance,
        margin_ratio_bps: margin_ratio_bps(equity, notional),
        liquidatable: equity < maintenance,
    }
}

/// The bankruptcy price beyond which equity goes negative (the insurance fund
/// must cover any shortfall past this point). Mode is informational here; the
/// computation is identical for cross and isolated single-position accounts.
#[must_use]
pub fn is_bankrupt(pos: &Position, mark: Price, _mode: MarginMode) -> bool {
    position_equity(pos, mark).is_negative()
}

/// Signed funding payment owed by a position for one accrual at `rate_bps`.
///
/// Positive funding rate => longs pay (positive result), shorts receive
/// (negative result). Returns the amount to add to `funding_paid`.
#[must_use]
pub fn funding_payment(pos: &Position, mark: Price, rate_bps: i64) -> Usd {
    let magnitude = pos.notional(mark).mul_bps(rate_bps);
    match pos.side {
        Side::Long => magnitude,
        Side::Short => -magnitude,
    }
}
