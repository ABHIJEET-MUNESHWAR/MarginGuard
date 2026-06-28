//! Engine and margin-math tests, including proptest solvency invariants.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use proptest::prelude::*;

use marginguard_core::margin;
use marginguard_core::{
    CommandOutcome, CoreError, EngineConfig, EventSink, PortError, PositionStore, RiskCommand,
    RiskEngine, RiskEvent,
};
use marginguard_types::{
    AccountId, Leverage, MarginMode, Position, Price, RiskParams, Side, Size, Symbol, Usd,
};

// ---- Test doubles ----------------------------------------------------------

#[derive(Default)]
struct MemStore {
    positions: Mutex<Vec<Position>>,
}

impl MemStore {
    fn snapshot(&self, symbol: &Symbol) -> Vec<Position> {
        self.positions
            .lock()
            .iter()
            .filter(|x| &x.symbol == symbol)
            .cloned()
            .collect()
    }
}

#[async_trait]
impl PositionStore for MemStore {
    async fn upsert(&self, position: Position) -> Result<(), PortError> {
        let mut p = self.positions.lock();
        p.retain(|x| !(x.account == position.account && x.symbol == position.symbol));
        p.push(position);
        Ok(())
    }
    async fn get(&self, account: &str, symbol: &Symbol) -> Result<Option<Position>, PortError> {
        Ok(self
            .positions
            .lock()
            .iter()
            .find(|x| x.account.as_str() == account && &x.symbol == symbol)
            .cloned())
    }
    async fn remove(&self, account: &str, symbol: &Symbol) -> Result<Option<Position>, PortError> {
        let mut p = self.positions.lock();
        if let Some(idx) = p
            .iter()
            .position(|x| x.account.as_str() == account && &x.symbol == symbol)
        {
            Ok(Some(p.remove(idx)))
        } else {
            Ok(None)
        }
    }
    async fn by_market(&self, symbol: &Symbol) -> Result<Vec<Position>, PortError> {
        Ok(self.snapshot(symbol))
    }
    async fn count(&self) -> Result<u64, PortError> {
        Ok(self.positions.lock().len() as u64)
    }
}

struct NoopSink;
#[async_trait]
impl EventSink for NoopSink {
    async fn publish(&self, _events: &[RiskEvent]) -> Result<(), PortError> {
        Ok(())
    }
}

struct FailingSink;
#[async_trait]
impl EventSink for FailingSink {
    async fn publish(&self, _events: &[RiskEvent]) -> Result<(), PortError> {
        Err(PortError::Unavailable("sink down".into()))
    }
}

/// Bundles an engine with a direct handle to its backing store so tests can
/// inspect surviving positions after a liquidation sweep.
struct Harness {
    engine: RiskEngine,
    store: Arc<MemStore>,
}

fn harness(sink: Arc<dyn EventSink>, insurance: Usd, config: EngineConfig) -> Harness {
    let store = Arc::new(MemStore::default());
    let engine = RiskEngine::new(store.clone(), sink, insurance, config);
    Harness { engine, store }
}

fn default_harness() -> Harness {
    harness(
        Arc::new(NoopSink),
        Usd::from_whole(1_000_000),
        EngineConfig::default(),
    )
}

fn sym() -> Symbol {
    Symbol::new("SOL-PERP").unwrap()
}

async fn set_mark(engine: &RiskEngine, price: i64, funding_bps: i64) {
    let p = Price::from_whole(price).unwrap();
    engine
        .apply(RiskCommand::UpdateMarket {
            symbol: sym(),
            mark_price: p,
            index_price: p,
            funding_rate_bps: funding_bps,
        })
        .await
        .unwrap();
}

fn open_cmd(account: &str, side: Side, entry: i64, size: i64, margin: i64) -> RiskCommand {
    RiskCommand::OpenPosition {
        account: AccountId::new(account).unwrap(),
        symbol: sym(),
        side,
        margin_mode: MarginMode::Cross,
        size: Size::from_whole(size).unwrap(),
        entry_price: Price::from_whole(entry).unwrap(),
        leverage: Leverage::new(10).unwrap(),
        margin: Usd::from_whole(margin),
    }
}

fn long_pos(entry: i64, size: i64, margin: i64) -> Position {
    Position {
        account: AccountId::new("a").unwrap(),
        symbol: sym(),
        side: Side::Long,
        margin_mode: MarginMode::Cross,
        size: Size::from_whole(size).unwrap(),
        entry_price: Price::from_whole(entry).unwrap(),
        leverage: Leverage::new(10).unwrap(),
        posted_margin: Usd::from_whole(margin),
        funding_paid: Usd::ZERO,
    }
}

// ---- Margin math -----------------------------------------------------------

#[test]
fn maintenance_margin_is_bps_of_notional() {
    let pos = long_pos(100, 10, 100);
    let params = RiskParams::standard(); // 2.5% maintenance
    let mark = Price::from_whole(100).unwrap();
    // notional 1000 -> maintenance 25
    assert_eq!(
        margin::maintenance_margin(&pos, mark, &params),
        Usd::from_whole(25)
    );
}

#[test]
fn liquidation_price_below_entry_for_long() {
    let pos = long_pos(100, 10, 100);
    let params = RiskParams::standard();
    let lp = margin::liquidation_price(&pos, &params).unwrap();
    assert!(lp.micros() < pos.entry_price.micros());
    assert!(margin::is_liquidatable(&pos, lp, &params));
}

// ---- Engine behaviour ------------------------------------------------------

#[tokio::test]
async fn open_requires_initial_margin() {
    let h = default_harness();
    set_mark(&h.engine, 100, 0).await;
    // notional 1000, initial 5% = 50; posting 40 must fail.
    let err = h
        .engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 40))
        .await
        .unwrap_err();
    assert_eq!(err, CoreError::InsufficientMargin);
    let ok = h
        .engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 60))
        .await
        .unwrap();
    assert_eq!(ok.events.len(), 1);
}

#[tokio::test]
async fn duplicate_open_is_rejected() {
    let h = default_harness();
    set_mark(&h.engine, 100, 0).await;
    h.engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 100))
        .await
        .unwrap();
    let err = h
        .engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 100))
        .await
        .unwrap_err();
    assert_eq!(err, CoreError::PositionExists);
}

#[tokio::test]
async fn close_unknown_position_errs() {
    let h = default_harness();
    set_mark(&h.engine, 100, 0).await;
    let err = h
        .engine
        .apply(RiskCommand::ClosePosition {
            account: AccountId::new("ghost").unwrap(),
            symbol: sym(),
        })
        .await
        .unwrap_err();
    assert_eq!(err, CoreError::PositionNotFound);
}

#[tokio::test]
async fn close_realizes_and_emits() {
    let h = default_harness();
    set_mark(&h.engine, 100, 0).await;
    h.engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 100))
        .await
        .unwrap();
    let out = h
        .engine
        .apply(RiskCommand::ClosePosition {
            account: AccountId::new("acct").unwrap(),
            symbol: sym(),
        })
        .await
        .unwrap();
    assert!(out
        .events
        .iter()
        .any(|e| matches!(e, RiskEvent::PositionClosed { .. })));
    assert_eq!(h.store.count().await.unwrap(), 0);
}

#[tokio::test]
async fn funding_longs_pay_shorts() {
    let h = default_harness();
    set_mark(&h.engine, 100, 100).await; // +1% funding
    h.engine
        .apply(open_cmd("long", Side::Long, 100, 10, 100))
        .await
        .unwrap();
    h.engine
        .apply(open_cmd("short", Side::Short, 100, 10, 100))
        .await
        .unwrap();
    let out = h
        .engine
        .apply(RiskCommand::AccrueFunding { symbol: sym() })
        .await
        .unwrap();
    let amounts: Vec<Usd> = out
        .events
        .iter()
        .filter_map(|e| match e {
            RiskEvent::FundingSettled { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(amounts.len(), 2);
    assert!(amounts.contains(&Usd::from_whole(10)));
    assert!(amounts.contains(&Usd::from_whole(-10)));
}

#[tokio::test]
async fn solvent_liquidation_credits_insurance_fund() {
    let h = default_harness();
    set_mark(&h.engine, 100, 0).await;
    h.engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 60))
        .await
        .unwrap();
    let before = h.engine.insurance_balance();
    set_mark(&h.engine, 96, 0).await;
    let out = h
        .engine
        .apply(RiskCommand::LiquidateMarket { symbol: sym() })
        .await
        .unwrap();
    assert_eq!(out.liquidation_count(), 1);
    assert!(h.engine.insurance_balance().micros() >= before.micros());
    assert_eq!(h.engine.stats().liquidations, 1);
    assert!(h.store.snapshot(&sym()).is_empty());
}

#[tokio::test]
async fn bankruptcy_draws_insurance_then_socializes() {
    let h = harness(
        Arc::new(NoopSink),
        Usd::from_whole(5),
        EngineConfig::default(),
    );
    set_mark(&h.engine, 100, 0).await;
    h.engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 60))
        .await
        .unwrap();
    // Crash mark to 90: equity = 60 + (90-100)*10 = -40 (bankrupt).
    set_mark(&h.engine, 90, 0).await;
    let out = h
        .engine
        .apply(RiskCommand::LiquidateMarket { symbol: sym() })
        .await
        .unwrap();
    assert_eq!(out.liquidation_count(), 1);
    assert_eq!(h.engine.insurance_balance(), Usd::ZERO);
    assert!(out
        .events
        .iter()
        .any(|e| matches!(e, RiskEvent::AutoDeleveraged { .. })));
    assert_eq!(h.engine.stats().adl_events, 1);
}

#[tokio::test]
async fn healthy_position_is_not_liquidated() {
    let h = default_harness();
    set_mark(&h.engine, 100, 0).await;
    h.engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 200))
        .await
        .unwrap();
    let out = h
        .engine
        .apply(RiskCommand::LiquidateMarket { symbol: sym() })
        .await
        .unwrap();
    assert_eq!(out.liquidation_count(), 0);
    assert_eq!(h.store.snapshot(&sym()).len(), 1);
}

#[tokio::test]
async fn close_requires_known_market() {
    let h = default_harness();
    // open computes initial margin from entry_price, so it succeeds with no mark.
    h.engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 100))
        .await
        .unwrap();
    let err = h
        .engine
        .apply(RiskCommand::ClosePosition {
            account: AccountId::new("acct").unwrap(),
            symbol: sym(),
        })
        .await
        .unwrap_err();
    assert_eq!(err, CoreError::MarketNotFound);
}

#[tokio::test]
async fn account_health_reports_liquidatable() {
    let h = default_harness();
    set_mark(&h.engine, 100, 0).await;
    h.engine
        .apply(open_cmd("acct", Side::Long, 100, 10, 60))
        .await
        .unwrap();
    set_mark(&h.engine, 96, 0).await;
    let health = h.engine.account_health("acct", &sym()).await.unwrap();
    assert!(health.liquidatable);
    assert!(health.margin_ratio_bps.is_some());
}

#[tokio::test]
async fn sink_failure_propagates() {
    let h = harness(
        Arc::new(FailingSink),
        Usd::from_whole(1_000),
        EngineConfig::default(),
    );
    let err = set_mark_result(&h.engine, 100).await.unwrap_err();
    assert!(matches!(err, CoreError::Port(_)));
}

async fn set_mark_result(engine: &RiskEngine, price: i64) -> Result<CommandOutcome, CoreError> {
    let p = Price::from_whole(price).unwrap();
    engine
        .apply(RiskCommand::UpdateMarket {
            symbol: sym(),
            mark_price: p,
            index_price: p,
            funding_rate_bps: 0,
        })
        .await
}

#[tokio::test]
async fn rate_limited_when_bucket_empty() {
    let config = EngineConfig {
        ingest_capacity: 1.0,
        ingest_refill_per_sec: 0.0,
        ..EngineConfig::default()
    };
    let h = harness(Arc::new(NoopSink), Usd::from_whole(1_000), config);
    let p = Price::from_whole(100).unwrap();
    let cmd = || RiskCommand::UpdateMarket {
        symbol: sym(),
        mark_price: p,
        index_price: p,
        funding_rate_bps: 0,
    };
    h.engine.apply(cmd()).await.unwrap();
    assert_eq!(
        h.engine.apply(cmd()).await.unwrap_err(),
        CoreError::RateLimited
    );
}

// ---- Solvency property tests ----------------------------------------------

proptest! {
    // INVARIANT: after liquidating a market, no surviving position breaches
    // maintenance margin at the prevailing mark.
    #[test]
    fn liquidation_clears_all_underwater_positions(
        entry in 50i64..200,
        crash in 1i64..49,
        size in 1i64..50,
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async move {
            let h = default_harness();
            set_mark(&h.engine, entry, 0).await;
            let notional = entry * size;
            let init_margin = ((notional * 5 + 99) / 100).max(1);
            h.engine
                .apply(open_cmd("acct", Side::Long, entry, size, init_margin))
                .await
                .unwrap();
            let crashed = (entry - crash).max(1);
            set_mark(&h.engine, crashed, 0).await;
            h.engine.apply(RiskCommand::LiquidateMarket { symbol: sym() }).await.unwrap();

            let params = RiskParams::standard();
            let mark = Price::from_whole(crashed).unwrap();
            for pos in h.store.snapshot(&sym()) {
                prop_assert!(!margin::is_liquidatable(&pos, mark, &params));
            }
            Ok(())
        }).unwrap();
    }

    // INVARIANT: funding is zero-sum across a matched long/short pair.
    #[test]
    fn funding_is_zero_sum(rate in -300i64..300, size in 1i64..100, entry in 10i64..500) {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async move {
            let h = default_harness();
            set_mark(&h.engine, entry, rate).await;
            let m = ((entry * size * 5 + 99) / 100).max(1);
            h.engine.apply(open_cmd("long", Side::Long, entry, size, m)).await.unwrap();
            h.engine.apply(open_cmd("short", Side::Short, entry, size, m)).await.unwrap();
            let out = h.engine.apply(RiskCommand::AccrueFunding { symbol: sym() }).await.unwrap();
            let sum: i128 = out.events.iter().filter_map(|e| match e {
                RiskEvent::FundingSettled { amount, .. } => Some(amount.micros()),
                _ => None,
            }).sum();
            prop_assert_eq!(sum, 0);
            Ok(())
        }).unwrap();
    }
}
