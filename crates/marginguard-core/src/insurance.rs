//! The insurance fund: absorbs liquidation shortfalls before losses are
//! socialised via auto-deleveraging.

use marginguard_types::Usd;

/// A thread-unsafe insurance-fund balance (guarded by the engine's lock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InsuranceFund {
    balance: Usd,
}

impl InsuranceFund {
    /// Create a fund seeded with `initial` balance.
    #[must_use]
    pub const fn new(initial: Usd) -> Self {
        InsuranceFund { balance: initial }
    }

    /// Current balance.
    #[must_use]
    pub const fn balance(self) -> Usd {
        self.balance
    }

    /// Credit the fund (e.g. liquidation fees or surplus from a solvent close).
    pub fn credit(&mut self, amount: Usd) {
        self.balance = self.balance.saturating_add(amount);
    }

    /// Attempt to cover a `shortfall`.
    ///
    /// Draws as much as the balance allows. Returns `(drawn, socialized)` where
    /// `drawn` is covered by the fund and `socialized` is the uncovered remainder
    /// that must be auto-deleveraged.
    pub fn cover(&mut self, shortfall: Usd) -> (Usd, Usd) {
        debug_assert!(!shortfall.is_negative());
        if self.balance >= shortfall {
            self.balance = self.balance.saturating_sub(shortfall);
            (shortfall, Usd::ZERO)
        } else {
            let drawn = self.balance.max(Usd::ZERO);
            let socialized = shortfall.saturating_sub(drawn);
            self.balance = Usd::ZERO;
            (drawn, socialized)
        }
    }
}

impl Default for InsuranceFund {
    fn default() -> Self {
        InsuranceFund::new(Usd::ZERO)
    }
}
