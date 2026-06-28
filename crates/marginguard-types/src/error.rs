//! Validation errors for constructing domain values.

use thiserror::Error;

/// An invalid market-data or position input.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidInput {
    /// A price was zero or negative.
    #[error("price must be positive")]
    NonPositivePrice,
    /// A price exceeded the permitted maximum.
    #[error("price exceeds maximum")]
    PriceTooLarge,
    /// A size was zero or negative.
    #[error("size must be positive")]
    NonPositiveSize,
    /// A size exceeded the permitted maximum.
    #[error("size exceeds maximum")]
    SizeTooLarge,
    /// Leverage was out of the allowed `1..=max` range.
    #[error("leverage must be between 1x and {max}x")]
    LeverageOutOfRange {
        /// The configured maximum leverage.
        max: u32,
    },
    /// A market symbol was empty or too long.
    #[error("symbol must be 1..={max} bytes")]
    InvalidSymbol {
        /// The maximum symbol length.
        max: usize,
    },
    /// An account id was empty or too long.
    #[error("account id must be 1..={max} bytes")]
    InvalidAccount {
        /// The maximum account-id length.
        max: usize,
    },
}

impl InvalidInput {
    /// A stable, machine-readable code for this error.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            InvalidInput::NonPositivePrice => "non_positive_price",
            InvalidInput::PriceTooLarge => "price_too_large",
            InvalidInput::NonPositiveSize => "non_positive_size",
            InvalidInput::SizeTooLarge => "size_too_large",
            InvalidInput::LeverageOutOfRange { .. } => "leverage_out_of_range",
            InvalidInput::InvalidSymbol { .. } => "invalid_symbol",
            InvalidInput::InvalidAccount { .. } => "invalid_account",
        }
    }
}
