//! A deterministic mark-price oracle simulator.
//!
//! Uses the `splitmix64` integer hash to generate a reproducible pseudo-random
//! walk with configurable drift and volatility — no RNG crate, so simulations
//! and tests are byte-for-byte repeatable from a seed.

/// A reproducible mark-price generator.
#[derive(Debug, Clone)]
pub struct SimMarketOracle {
    state: u64,
    price_micros: i128,
    drift_bps: i64,
    vol_bps: i64,
}

impl SimMarketOracle {
    /// Create an oracle starting at `start_price_micros`, applying `drift_bps`
    /// expected change and up to `vol_bps` symmetric noise per step.
    #[must_use]
    pub fn new(seed: u64, start_price_micros: i128, drift_bps: i64, vol_bps: i64) -> Self {
        SimMarketOracle {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
            price_micros: start_price_micros.max(1),
            drift_bps,
            vol_bps: vol_bps.max(0),
        }
    }

    fn next_u64(&mut self) -> u64 {
        // splitmix64
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The current mark price in micro-USD.
    #[must_use]
    pub fn price(&self) -> i128 {
        self.price_micros
    }

    /// Advance one step and return the new mark price in micro-USD (>= 1).
    pub fn next_price(&mut self) -> i128 {
        let noise = if self.vol_bps == 0 {
            0
        } else {
            let span = 2 * self.vol_bps + 1;
            (self.next_u64() % span as u64) as i64 - self.vol_bps
        };
        let change_bps = self.drift_bps + noise;
        let delta = self.price_micros * i128::from(change_bps) / 10_000;
        self.price_micros = (self.price_micros + delta).max(1);
        self.price_micros
    }
}
