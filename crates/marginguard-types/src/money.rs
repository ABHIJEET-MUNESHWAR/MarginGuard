//! Fixed-point monetary arithmetic.
//!
//! All money and price quantities are represented as **signed `i128`
//! micro-units** (six implied decimal places, `SCALE = 1_000_000`). Using a
//! fixed-point integer instead of `f64` makes margin math exact, deterministic,
//! and free of rounding drift — essential for a solvency-critical engine.
//!
//! Construction is bounded so that the products that appear in margin math
//! (`price * size`) cannot overflow `i128`.

use serde::{Deserialize, Serialize};

/// Number of micro-units per whole unit (six decimal places).
pub const SCALE: i128 = 1_000_000;

/// Maximum absolute price in micro-USD (`$10_000_000`).
pub const MAX_PRICE: i128 = 10_000_000 * SCALE;

/// Maximum absolute size in micro-contracts (`10_000_000` contracts).
pub const MAX_SIZE: i128 = 10_000_000 * SCALE;

/// A signed USD amount in micro-dollars (6 dp).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Usd(i128);

impl Usd {
    /// Zero dollars.
    pub const ZERO: Usd = Usd(0);

    /// Construct from raw micro-USD.
    #[must_use]
    pub const fn from_micros(micros: i128) -> Self {
        Usd(micros)
    }

    /// Construct from a whole-dollar amount.
    #[must_use]
    pub const fn from_whole(dollars: i64) -> Self {
        Usd(dollars as i128 * SCALE)
    }

    /// Raw micro-USD value.
    #[must_use]
    pub const fn micros(self) -> i128 {
        self.0
    }

    /// Whether the amount is negative.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Saturating addition.
    #[must_use]
    pub const fn saturating_add(self, other: Usd) -> Usd {
        Usd(self.0.saturating_add(other.0))
    }

    /// Saturating subtraction.
    #[must_use]
    pub const fn saturating_sub(self, other: Usd) -> Usd {
        Usd(self.0.saturating_sub(other.0))
    }

    /// Multiply by basis points (`bps`/10_000), rounding toward zero.
    #[must_use]
    pub const fn mul_bps(self, bps: i64) -> Usd {
        Usd(self.0.saturating_mul(bps as i128) / 10_000)
    }

    /// The larger of two amounts.
    #[must_use]
    pub fn max(self, other: Usd) -> Usd {
        Usd(self.0.max(other.0))
    }

    /// The smaller of two amounts.
    #[must_use]
    pub fn min(self, other: Usd) -> Usd {
        Usd(self.0.min(other.0))
    }
}

impl std::fmt::Display for Usd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let whole = self.0 / SCALE;
        let frac = (self.0 % SCALE).abs();
        write!(f, "{whole}.{frac:06} USD")
    }
}

impl std::ops::Add for Usd {
    type Output = Usd;
    fn add(self, rhs: Usd) -> Usd {
        self.saturating_add(rhs)
    }
}

impl std::ops::Sub for Usd {
    type Output = Usd;
    fn sub(self, rhs: Usd) -> Usd {
        self.saturating_sub(rhs)
    }
}

impl std::ops::Neg for Usd {
    type Output = Usd;
    fn neg(self) -> Usd {
        Usd(self.0.saturating_neg())
    }
}

/// Compute `value * size / SCALE` in `i128` with saturating multiplication.
///
/// Used to turn a per-contract `price` and a `size` (both 6-dp micro-units)
/// into a notional micro-USD amount at the correct scale.
#[must_use]
pub const fn scaled_mul(value: i128, size: i128) -> i128 {
    value.saturating_mul(size) / SCALE
}
