//! Criterion micro-benchmarks for the risk hot paths: pure margin math and the
//! liquidation-waterfall scan over a market.

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use tokio::runtime::Runtime;

use marginguard_core::{margin, EngineConfig, RiskCommand, RiskEngine};
use marginguard_infra::{BroadcastEventSink, MemoryPositionStore};
use marginguard_types::{
    AccountId, Leverage, MarginMode, Position, Price, RiskParams, Side, Size, Symbol, Usd,
};

fn sym() -> Symbol {
    Symbol::new("SOL-PERP").unwrap()
}

fn long_pos(entry: i64, size: i64, margin: i64) -> Position {
    Position {
        account: AccountId::new("acct").unwrap(),
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

fn bench_margin_math(c: &mut Criterion) {
    let pos = long_pos(100, 10, 60);
    let params = RiskParams::standard();
    let mark = Price::from_whole(96).unwrap();

    let mut group = c.benchmark_group("margin");
    group.throughput(Throughput::Elements(1));
    group.bench_function("account_health", |b| {
        b.iter(|| margin::account_health(&pos, mark, &params));
    });
    group.bench_function("liquidation_price", |b| {
        b.iter(|| margin::liquidation_price(&pos, &params));
    });
    group.finish();
}

fn underwater_engine(rt: &Runtime, positions: u64) -> RiskEngine {
    let store = Arc::new(MemoryPositionStore::new());
    let bus = Arc::new(BroadcastEventSink::new(16));
    let engine = RiskEngine::new(
        store,
        bus,
        Usd::from_whole(1_000_000_000),
        EngineConfig::default(),
    );
    rt.block_on(async {
        let set = |price: i64| RiskCommand::UpdateMarket {
            symbol: sym(),
            mark_price: Price::from_whole(price).unwrap(),
            index_price: Price::from_whole(price).unwrap(),
            funding_rate_bps: 0,
        };
        engine.apply(set(100)).await.unwrap();
        for i in 0..positions {
            engine
                .apply(RiskCommand::OpenPosition {
                    account: AccountId::new(format!("acct-{i}")).unwrap(),
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
        }
        // Crash the mark so every position is liquidatable.
        engine.apply(set(90)).await.unwrap();
    });
    engine
}

fn bench_liquidation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("liquidation");
    for &k in &[10u64, 100, 500] {
        group.throughput(Throughput::Elements(k));
        group.bench_function(format!("liquidate_market_{k}"), |b| {
            b.iter_batched(
                || underwater_engine(&rt, k),
                |engine| {
                    rt.block_on(async {
                        engine
                            .apply(RiskCommand::LiquidateMarket { symbol: sym() })
                            .await
                            .unwrap()
                    });
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_margin_math, bench_liquidation);
criterion_main!(benches);
