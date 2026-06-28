//! Deterministic market-scenario simulator: opens a book of positions, walks
//! the mark price, accrues funding, runs the liquidation waterfall each step,
//! and prints a risk report. Showcases the engine, insurance fund, ADL, and
//! the AI advisor end-to-end with no external dependencies.

use std::sync::Arc;

use marginguard_ai::AdviceContext;
use marginguard_core::{RiskCommand, RiskEngine};
use marginguard_infra::{BroadcastEventSink, MemoryPositionStore, SimMarketOracle};
use marginguard_types::{AccountId, Leverage, MarginMode, Price, Side, Size, Symbol, Usd};

use crate::config::SimulateArgs;
use crate::startup::build_advisor;

/// Aggregate results of a simulation run.
#[derive(Debug, Clone, Copy, Default)]
pub struct SimReport {
    /// Steps walked.
    pub steps: u64,
    /// Accounts opened.
    pub accounts: u64,
    /// Liquidations performed.
    pub liquidations: u64,
    /// Auto-deleverage (socialised-loss) events.
    pub adl_events: u64,
    /// Funding settlements applied.
    pub funding_settlements: u64,
    /// Insurance balance at start, in micro-USD.
    pub insurance_start_micros: i128,
    /// Insurance balance at end, in micro-USD.
    pub insurance_end_micros: i128,
    /// Start mark price in micro-USD.
    pub start_price_micros: i128,
    /// Final mark price in micro-USD.
    pub final_price_micros: i128,
    /// Positions still open at the end.
    pub survivors: u64,
}

/// Run the scenario described by `args` and return a report.
///
/// # Errors
/// Propagates engine errors (validation, port failures).
pub async fn run_simulation(args: SimulateArgs) -> anyhow::Result<SimReport> {
    let store = Arc::new(MemoryPositionStore::new());
    let bus = Arc::new(BroadcastEventSink::new(4096));
    let engine = RiskEngine::new(
        store,
        bus,
        Usd::from_whole(i64::try_from(args.insurance_seed).unwrap_or(i64::MAX)),
        marginguard_core::EngineConfig::default(),
    );
    let symbol = Symbol::new("SOL-PERP").map_err(to_anyhow)?;

    let start_price = Price::from_whole(args.start_price).map_err(to_anyhow)?;
    set_market(&engine, &symbol, start_price, args.funding_bps).await?;
    let insurance_start = engine.insurance_balance().micros();

    // Open a book: alternating longs and shorts, thinly margined at ~6% so a
    // moderate adverse move pushes the wrong side underwater.
    let size = 10i64;
    let notional = args.start_price * size;
    let margin_whole = (notional * 6) / 100;
    for i in 0..args.accounts {
        let side = if i % 2 == 0 { Side::Long } else { Side::Short };
        let account = AccountId::new(format!("acct-{i}")).map_err(to_anyhow)?;
        engine
            .apply(RiskCommand::OpenPosition {
                account,
                symbol: symbol.clone(),
                side,
                margin_mode: MarginMode::Cross,
                size: Size::from_whole(size).map_err(to_anyhow)?,
                entry_price: start_price,
                leverage: Leverage::new(10).map_err(to_anyhow)?,
                margin: Usd::from_whole(margin_whole),
            })
            .await?;
    }

    let mut oracle = SimMarketOracle::new(
        args.seed,
        start_price.micros(),
        args.drift_bps,
        args.vol_bps,
    );
    for step in 0..args.steps {
        let price_micros = oracle.next_price();
        let price = Price::from_micros(price_micros).map_err(to_anyhow)?;
        set_market(&engine, &symbol, price, args.funding_bps).await?;
        if args.funding_interval > 0 && step % args.funding_interval == 0 {
            engine
                .apply(RiskCommand::AccrueFunding {
                    symbol: symbol.clone(),
                })
                .await?;
        }
        engine
            .apply(RiskCommand::LiquidateMarket {
                symbol: symbol.clone(),
            })
            .await?;
    }

    let stats = engine.stats();
    let survivors = engine.positions_in(&symbol).await?.len() as u64;

    // Sample advice for a surviving account, if any (exercises the AI path).
    print_sample_advice(&engine, &symbol, &args).await;

    Ok(SimReport {
        steps: args.steps,
        accounts: args.accounts,
        liquidations: stats.liquidations,
        adl_events: stats.adl_events,
        funding_settlements: stats.funding_settlements,
        insurance_start_micros: insurance_start,
        insurance_end_micros: engine.insurance_balance().micros(),
        start_price_micros: start_price.micros(),
        final_price_micros: oracle.price(),
        survivors,
    })
}

async fn set_market(
    engine: &RiskEngine,
    symbol: &Symbol,
    price: Price,
    funding_bps: i64,
) -> anyhow::Result<()> {
    engine
        .apply(RiskCommand::UpdateMarket {
            symbol: symbol.clone(),
            mark_price: price,
            index_price: price,
            funding_rate_bps: funding_bps,
        })
        .await?;
    Ok(())
}

async fn print_sample_advice(engine: &RiskEngine, symbol: &Symbol, args: &SimulateArgs) {
    let advisor = build_advisor(args.advisor);
    for i in 0..args.accounts {
        let account = format!("acct-{i}");
        let Ok(Some(pos)) = engine.position(&account, symbol).await else {
            continue;
        };
        let Ok(health) = engine.account_health(&account, symbol).await else {
            continue;
        };
        let Some(market) = engine.market(symbol) else {
            continue;
        };
        let advice = advisor
            .assess(&AdviceContext::from_parts(&pos, &health, &market))
            .await;
        println!(
            "│ sample advice ({account}) [{}/{}]: {}",
            advice.risk_level.code(),
            advice.source.code(),
            advice.summary
        );
        return;
    }
}

fn to_anyhow(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!(e.to_string())
}

/// Print a human-readable simulation report.
pub fn print_report(r: &SimReport) {
    let usd = |micros: i128| micros as f64 / 1_000_000.0;
    println!("┌─ MarginGuard simulation report ────────────────");
    println!("│ steps              : {}", r.steps);
    println!("│ accounts opened    : {}", r.accounts);
    println!("│ survivors          : {}", r.survivors);
    println!("│ liquidations       : {}", r.liquidations);
    println!("│ ADL events         : {}", r.adl_events);
    println!("│ funding settlements: {}", r.funding_settlements);
    println!(
        "│ mark price         : {:.2} -> {:.2} USD",
        usd(r.start_price_micros),
        usd(r.final_price_micros)
    );
    println!(
        "│ insurance fund     : {:.2} -> {:.2} USD",
        usd(r.insurance_start_micros),
        usd(r.insurance_end_micros)
    );
    println!("└────────────────────────────────────────────────");
}
