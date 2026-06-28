//! Infra adapter tests: store semantics, broadcast fan-out, and a full
//! engine-over-adapters pipeline that drives real liquidations.

use std::sync::Arc;

use futures::StreamExt;

use marginguard_core::{EngineConfig, RiskCommand, RiskEngine, RiskEvent, RiskEventStream};
use marginguard_infra::{BroadcastEventSink, MemoryPositionStore, SimMarketOracle};
use marginguard_types::{
    AccountId, Leverage, MarginMode, Position, Price, Side, Size, Symbol, Usd,
};

fn sym() -> Symbol {
    Symbol::new("SOL-PERP").unwrap()
}

fn pos(account: &str, entry: i64, size: i64, margin: i64) -> Position {
    Position {
        account: AccountId::new(account).unwrap(),
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

#[tokio::test]
async fn memory_store_crud_and_index() {
    use marginguard_core::PositionStore;
    let store = MemoryPositionStore::new();
    store.upsert(pos("alice", 100, 10, 100)).await.unwrap();
    store.upsert(pos("bob", 100, 5, 50)).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 2);
    assert_eq!(store.by_market(&sym()).await.unwrap().len(), 2);

    let got = store.get("alice", &sym()).await.unwrap().unwrap();
    assert_eq!(got.account.as_str(), "alice");

    // Upsert replaces in place (no duplicate).
    store.upsert(pos("alice", 110, 20, 200)).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 2);
    assert_eq!(
        store.get("alice", &sym()).await.unwrap().unwrap().size,
        Size::from_whole(20).unwrap()
    );

    let removed = store.remove("alice", &sym()).await.unwrap().unwrap();
    assert_eq!(removed.account.as_str(), "alice");
    assert_eq!(store.count().await.unwrap(), 1);
    assert!(store.get("alice", &sym()).await.unwrap().is_none());
}

#[tokio::test]
async fn empty_market_returns_no_positions() {
    use marginguard_core::PositionStore;
    let store = MemoryPositionStore::new();
    assert!(store.by_market(&sym()).await.unwrap().is_empty());
}

#[tokio::test]
async fn broadcast_sink_fans_out_to_subscribers() {
    use marginguard_core::EventSink;
    let sink = BroadcastEventSink::new(16);
    let mut sub = sink.subscribe();
    sink.publish(&[RiskEvent::AutoDeleveraged {
        symbol: sym(),
        socialized_loss: Usd::from_whole(7),
    }])
    .await
    .unwrap();
    let ev = sub.next().await.unwrap();
    assert_eq!(ev.kind(), "auto_deleveraged");
}

#[tokio::test]
async fn publish_with_no_subscribers_is_ok() {
    use marginguard_core::EventSink;
    let sink = BroadcastEventSink::new(16);
    assert_eq!(sink.subscriber_count(), 0);
    sink.publish(&[RiskEvent::MarketUpdated {
        symbol: sym(),
        mark_price: 100_000_000,
        funding_rate_bps: 0,
    }])
    .await
    .unwrap();
}

#[test]
fn oracle_is_deterministic_for_a_seed() {
    let mut a = SimMarketOracle::new(42, 100_000_000, -20, 50);
    let mut b = SimMarketOracle::new(42, 100_000_000, -20, 50);
    for _ in 0..100 {
        assert_eq!(a.next_price(), b.next_price());
    }
    assert!(a.price() > 0);
}

#[tokio::test]
async fn engine_over_adapters_liquidates_on_crash() {
    let store = Arc::new(MemoryPositionStore::new());
    let sink = Arc::new(BroadcastEventSink::new(64));
    let mut events = sink.subscribe();
    let engine = RiskEngine::new(
        store.clone(),
        sink.clone(),
        Usd::from_whole(1_000),
        EngineConfig::default(),
    );

    // Establish a market and open a thinly-margined long.
    let set_mark = |price: i64| RiskCommand::UpdateMarket {
        symbol: sym(),
        mark_price: Price::from_whole(price).unwrap(),
        index_price: Price::from_whole(price).unwrap(),
        funding_rate_bps: 0,
    };
    engine.apply(set_mark(100)).await.unwrap();
    engine
        .apply(RiskCommand::OpenPosition {
            account: AccountId::new("acct").unwrap(),
            symbol: sym(),
            side: Side::Long,
            margin_mode: MarginMode::Cross,
            size: Size::from_whole(10).unwrap(),
            entry_price: Price::from_whole(100).unwrap(),
            leverage: Leverage::new(10).unwrap(),
            margin: Usd::from_whole(60),
        })
        .await
        .unwrap();

    // Walk the mark price down with a bearish oracle until liquidation.
    let mut oracle = SimMarketOracle::new(7, 100_000_000, -400, 30);
    let mut liquidated = false;
    for _ in 0..50 {
        let price = oracle.next_price();
        engine
            .apply(RiskCommand::UpdateMarket {
                symbol: sym(),
                mark_price: Price::from_micros(price).unwrap(),
                index_price: Price::from_micros(price).unwrap(),
                funding_rate_bps: 0,
            })
            .await
            .unwrap();
        let out = engine
            .apply(RiskCommand::LiquidateMarket { symbol: sym() })
            .await
            .unwrap();
        if out.liquidation_count() > 0 {
            liquidated = true;
            break;
        }
    }
    assert!(liquidated, "bearish walk should liquidate the long");
    assert_eq!(store.positions_len().await, 0);

    // The fan-out should have observed at least one liquidation event.
    let mut saw_liquidation = false;
    while let Ok(Some(ev)) =
        tokio::time::timeout(std::time::Duration::from_millis(50), events.next()).await
    {
        if ev.kind() == "liquidated" {
            saw_liquidation = true;
            break;
        }
    }
    assert!(saw_liquidation);
}

// Small extension trait so the test can assert the store is empty without
// importing the port trait at the top level.
trait StoreLen {
    async fn positions_len(&self) -> u64;
}

impl StoreLen for MemoryPositionStore {
    async fn positions_len(&self) -> u64 {
        use marginguard_core::PositionStore;
        self.count().await.unwrap()
    }
}
