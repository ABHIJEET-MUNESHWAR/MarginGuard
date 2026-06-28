//! Strongly-typed domain identifiers and bounded quantities.

use serde::{Deserialize, Serialize};

use crate::error::InvalidInput;
use crate::money::{MAX_PRICE, MAX_SIZE, SCALE};

/// Maximum length of a market symbol.
pub const MAX_SYMBOL_LEN: usize = 24;

/// Maximum length of an account id.
pub const MAX_ACCOUNT_LEN: usize = 48;

/// Maximum permitted leverage.
pub const MAX_LEVERAGE: u32 = 100;

/// A positive, bounded per-contract price in micro-USD (6 dp).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Price(i128);

impl Price {
    /// Construct from raw micro-USD, enforcing positivity and bounds.
    ///
    /// # Errors
    /// Returns [`InvalidInput`] if the price is non-positive or too large.
    pub const fn from_micros(micros: i128) -> Result<Self, InvalidInput> {
        if micros <= 0 {
            return Err(InvalidInput::NonPositivePrice);
        }
        if micros > MAX_PRICE {
            return Err(InvalidInput::PriceTooLarge);
        }
        Ok(Price(micros))
    }

    /// Construct from a whole-dollar price.
    ///
    /// # Errors
    /// Returns [`InvalidInput`] if the price is non-positive or too large.
    pub const fn from_whole(dollars: i64) -> Result<Self, InvalidInput> {
        Price::from_micros(dollars as i128 * SCALE)
    }

    /// Raw micro-USD value.
    #[must_use]
    pub const fn micros(self) -> i128 {
        self.0
    }
}

/// A positive, bounded position size in micro-contracts (6 dp).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Size(i128);

impl Size {
    /// Construct from raw micro-contracts, enforcing positivity and bounds.
    ///
    /// # Errors
    /// Returns [`InvalidInput`] if the size is non-positive or too large.
    pub const fn from_micros(micros: i128) -> Result<Self, InvalidInput> {
        if micros <= 0 {
            return Err(InvalidInput::NonPositiveSize);
        }
        if micros > MAX_SIZE {
            return Err(InvalidInput::SizeTooLarge);
        }
        Ok(Size(micros))
    }

    /// Construct from a whole-contract count.
    ///
    /// # Errors
    /// Returns [`InvalidInput`] if the size is non-positive or too large.
    pub const fn from_whole(contracts: i64) -> Result<Self, InvalidInput> {
        Size::from_micros(contracts as i128 * SCALE)
    }

    /// Raw micro-contract value.
    #[must_use]
    pub const fn micros(self) -> i128 {
        self.0
    }
}

/// Leverage multiplier in the inclusive range `1..=MAX_LEVERAGE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Leverage(u32);

impl Leverage {
    /// Construct a leverage multiplier.
    ///
    /// # Errors
    /// Returns [`InvalidInput::LeverageOutOfRange`] outside `1..=MAX_LEVERAGE`.
    pub const fn new(x: u32) -> Result<Self, InvalidInput> {
        if x == 0 || x > MAX_LEVERAGE {
            return Err(InvalidInput::LeverageOutOfRange { max: MAX_LEVERAGE });
        }
        Ok(Leverage(x))
    }

    /// The leverage multiplier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A bounded market symbol such as `SOL-PERP`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Symbol(String);

impl Symbol {
    /// Construct a symbol, enforcing a `1..=MAX_SYMBOL_LEN` length.
    ///
    /// # Errors
    /// Returns [`InvalidInput::InvalidSymbol`] if the length is out of range.
    pub fn new(s: impl Into<String>) -> Result<Self, InvalidInput> {
        let s = s.into();
        if s.is_empty() || s.len() > MAX_SYMBOL_LEN {
            return Err(InvalidInput::InvalidSymbol {
                max: MAX_SYMBOL_LEN,
            });
        }
        Ok(Symbol(s))
    }

    /// The symbol as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A bounded account identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(String);

impl AccountId {
    /// Construct an account id, enforcing a `1..=MAX_ACCOUNT_LEN` length.
    ///
    /// # Errors
    /// Returns [`InvalidInput::InvalidAccount`] if the length is out of range.
    pub fn new(s: impl Into<String>) -> Result<Self, InvalidInput> {
        let s = s.into();
        if s.is_empty() || s.len() > MAX_ACCOUNT_LEN {
            return Err(InvalidInput::InvalidAccount {
                max: MAX_ACCOUNT_LEN,
            });
        }
        Ok(AccountId(s))
    }

    /// The account id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
