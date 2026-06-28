//! # marginguard-types
//!
//! The domain vocabulary for MarginGuard's perpetual-futures risk engine.
//! Pure data and validation only — no I/O, no async, no business process.
//!
//! Money and prices are exact fixed-point [`money::Usd`] / [`ids::Price`]
//! values (6-dp `i128` micro-units), and invariants such as positivity and
//! bounds are pushed to construction time so illegal states are unrepresentable.

#![forbid(unsafe_code)]

pub mod error;
pub mod ids;
pub mod market;
pub mod money;
pub mod position;
pub mod risk;

pub use error::InvalidInput;
pub use ids::{
    AccountId, Leverage, Price, Size, Symbol, MAX_ACCOUNT_LEN, MAX_LEVERAGE, MAX_SYMBOL_LEN,
};
pub use market::{MarketState, RiskParams};
pub use money::{scaled_mul, Usd, MAX_PRICE, MAX_SIZE, SCALE};
pub use position::{MarginMode, Position, Side};
pub use risk::{AccountHealth, Liquidation, LiquidationReason, RiskStats};
