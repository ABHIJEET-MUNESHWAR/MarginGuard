//! Unit and property tests for the domain types.

use marginguard_types::{
    AccountId, InvalidInput, Leverage, MarginMode, Position, Price, Side, Size, Symbol, Usd,
};
use proptest::prelude::*;

fn position(side: Side, entry: i64, size: i64) -> Position {
    Position {
        account: AccountId::new("acct-1").unwrap(),
        symbol: Symbol::new("SOL-PERP").unwrap(),
        side,
        margin_mode: MarginMode::Cross,
        size: Size::from_whole(size).unwrap(),
        entry_price: Price::from_whole(entry).unwrap(),
        leverage: Leverage::new(10).unwrap(),
        posted_margin: Usd::from_whole(1_000),
        funding_paid: Usd::ZERO,
    }
}

#[test]
fn price_rejects_non_positive_and_oversize() {
    assert_eq!(Price::from_micros(0), Err(InvalidInput::NonPositivePrice));
    assert_eq!(Price::from_micros(-1), Err(InvalidInput::NonPositivePrice));
    assert_eq!(
        Price::from_whole(20_000_000),
        Err(InvalidInput::PriceTooLarge)
    );
    assert!(Price::from_whole(150).is_ok());
}

#[test]
fn size_rejects_non_positive_and_oversize() {
    assert_eq!(Size::from_micros(0), Err(InvalidInput::NonPositiveSize));
    assert_eq!(
        Size::from_whole(20_000_000),
        Err(InvalidInput::SizeTooLarge)
    );
    assert!(Size::from_whole(5).is_ok());
}

#[test]
fn leverage_bounds_enforced() {
    assert!(matches!(
        Leverage::new(0),
        Err(InvalidInput::LeverageOutOfRange { .. })
    ));
    assert!(matches!(
        Leverage::new(101),
        Err(InvalidInput::LeverageOutOfRange { .. })
    ));
    assert_eq!(Leverage::new(25).unwrap().get(), 25);
}

#[test]
fn symbol_and_account_length_bounds() {
    assert!(Symbol::new("").is_err());
    assert!(Symbol::new("X".repeat(25)).is_err());
    assert_eq!(Symbol::new("BTC-PERP").unwrap().as_str(), "BTC-PERP");

    assert!(AccountId::new("").is_err());
    assert!(AccountId::new("a".repeat(49)).is_err());
}

#[test]
fn long_pnl_is_positive_when_mark_rises() {
    let pos = position(Side::Long, 100, 10);
    let pnl = pos.unrealized_pnl(Price::from_whole(110).unwrap());
    // (110 - 100) * 10 = 100 USD
    assert_eq!(pnl, Usd::from_whole(100));
}

#[test]
fn short_pnl_is_positive_when_mark_falls() {
    let pos = position(Side::Short, 100, 10);
    let pnl = pos.unrealized_pnl(Price::from_whole(90).unwrap());
    // (100 - 90) * 10 = 100 USD
    assert_eq!(pnl, Usd::from_whole(100));
}

#[test]
fn notional_scales_price_times_size() {
    let pos = position(Side::Long, 100, 10);
    assert_eq!(
        pos.notional(Price::from_whole(120).unwrap()),
        Usd::from_whole(1_200)
    );
}

#[test]
fn side_sign_and_opposite() {
    assert_eq!(Side::Long.sign(), 1);
    assert_eq!(Side::Short.sign(), -1);
    assert_eq!(Side::Long.opposite(), Side::Short);
    assert_eq!(Side::Long.code(), "long");
    assert_eq!(MarginMode::Cross.code(), "cross");
}

#[test]
fn usd_arithmetic_saturates() {
    let a = Usd::from_whole(100);
    let b = Usd::from_whole(40);
    assert_eq!(a.saturating_sub(b), Usd::from_whole(60));
    assert_eq!(
        a.mul_bps(250),
        Usd::from_whole(2) + Usd::from_micros(500_000)
    );
    assert!(Usd::from_whole(-5).is_negative());
    assert_eq!(-Usd::from_whole(7), Usd::from_whole(-7));
}

#[test]
fn invalid_input_codes_are_stable() {
    assert_eq!(InvalidInput::NonPositivePrice.code(), "non_positive_price");
    assert_eq!(
        InvalidInput::LeverageOutOfRange { max: 100 }.code(),
        "leverage_out_of_range"
    );
}

proptest::proptest! {
    #[test]
    fn pnl_is_antisymmetric_in_side(entry in 1i64..1_000, mark in 1i64..1_000, size in 1i64..1_000) {
        let long = position(Side::Long, entry, size);
        let short = position(Side::Short, entry, size);
        let m = Price::from_whole(mark).unwrap();
        // Long PnL == -(Short PnL) for the same entry/mark/size.
        prop_assert_eq!(long.unrealized_pnl(m).micros(), -short.unrealized_pnl(m).micros());
    }

    #[test]
    fn notional_is_non_negative(entry in 1i64..1_000, mark in 1i64..1_000, size in 1i64..1_000) {
        let pos = position(Side::Long, entry, size);
        let m = Price::from_whole(mark).unwrap();
        prop_assert!(pos.notional(m).micros() >= 0);
    }
}
