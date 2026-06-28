//! End-to-end GraphQL tests over an in-memory engine: queries, mutations, the
//! AI advice resolver, and a live subscription.

use std::sync::Arc;
use std::time::Duration;

use async_graphql::Request;
use futures::StreamExt;

use marginguard_ai::HeuristicAdvisor;
use marginguard_api::{build_schema, ApiContext, MarginGuardSchema};
use marginguard_core::{EngineConfig, RiskEngine};
use marginguard_infra::{BroadcastEventSink, MemoryPositionStore};
use marginguard_types::Usd;

fn schema_with_engine() -> (MarginGuardSchema, RiskEngine) {
    let store = Arc::new(MemoryPositionStore::new());
    let bus = Arc::new(BroadcastEventSink::new(64));
    let engine = RiskEngine::new(
        store,
        bus.clone(),
        Usd::from_whole(1_000),
        EngineConfig::default(),
    );
    let advisor = Arc::new(HeuristicAdvisor::new());
    let context = ApiContext::new(engine.clone(), advisor, bus);
    (build_schema(context), engine)
}

async fn run(schema: &MarginGuardSchema, query: &str) -> serde_json::Value {
    let resp = schema.execute(Request::new(query)).await;
    assert!(resp.errors.is_empty(), "graphql errors: {:?}", resp.errors);
    resp.data.into_json().unwrap()
}

#[tokio::test]
async fn api_version_is_exposed() {
    let (schema, _engine) = schema_with_engine();
    let data = run(&schema, "{ apiVersion }").await;
    assert_eq!(data["apiVersion"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn open_then_query_position_and_health() {
    let (schema, _engine) = schema_with_engine();
    run(
        &schema,
        r#"mutation { updateMarket(symbol:"SOL-PERP", markPrice:100, indexPrice:100, fundingRateBps:0) { liquidationCount } }"#,
    )
    .await;
    let opened = run(
        &schema,
        r#"mutation {
            openPosition(input:{
                account:"acct", symbol:"SOL-PERP", side:LONG, marginMode:CROSS,
                size:10, entryPrice:100, leverage:10, margin:60
            }) { liquidationCount events { kind } }
        }"#,
    )
    .await;
    assert_eq!(opened["openPosition"]["liquidationCount"], 0);
    assert_eq!(
        opened["openPosition"]["events"][0]["kind"],
        "position_opened"
    );

    let pos = run(
        &schema,
        r#"{ position(account:"acct", symbol:"SOL-PERP") { account leverage entryPrice { value } } }"#,
    )
    .await;
    assert_eq!(pos["position"]["account"], "acct");
    assert_eq!(pos["position"]["leverage"], 10);
    assert_eq!(pos["position"]["entryPrice"]["value"], 100.0);

    let health = run(
        &schema,
        r#"{ accountHealth(account:"acct", symbol:"SOL-PERP") { liquidatable marginRatioBps } }"#,
    )
    .await;
    assert_eq!(health["accountHealth"]["liquidatable"], false);
}

#[tokio::test]
async fn risk_advice_resolver_returns_tier() {
    let (schema, _engine) = schema_with_engine();
    run(
        &schema,
        r#"mutation { updateMarket(symbol:"SOL-PERP", markPrice:100, indexPrice:100, fundingRateBps:0) { liquidationCount } }"#,
    )
    .await;
    run(
        &schema,
        r#"mutation { openPosition(input:{account:"acct", symbol:"SOL-PERP", side:LONG, marginMode:CROSS, size:10, entryPrice:100, leverage:10, margin:60}) { liquidationCount } }"#,
    )
    .await;
    let advice = run(
        &schema,
        r#"{ riskAdvice(account:"acct", symbol:"SOL-PERP") { riskLevel source confidence } }"#,
    )
    .await;
    assert_eq!(advice["riskAdvice"]["source"], "heuristic");
    let level = advice["riskAdvice"]["riskLevel"].as_str().unwrap();
    assert!(["safe", "caution", "warning", "critical"].contains(&level));
}

#[tokio::test]
async fn liquidation_waterfall_via_graphql() {
    let (schema, _engine) = schema_with_engine();
    run(
        &schema,
        r#"mutation { updateMarket(symbol:"SOL-PERP", markPrice:100, indexPrice:100, fundingRateBps:0) { liquidationCount } }"#,
    )
    .await;
    run(
        &schema,
        r#"mutation { openPosition(input:{account:"acct", symbol:"SOL-PERP", side:LONG, marginMode:CROSS, size:10, entryPrice:100, leverage:10, margin:60}) { liquidationCount } }"#,
    )
    .await;
    // Crash the mark below maintenance.
    run(
        &schema,
        r#"mutation { updateMarket(symbol:"SOL-PERP", markPrice:96, indexPrice:96, fundingRateBps:0) { liquidationCount } }"#,
    )
    .await;
    let liq = run(
        &schema,
        r#"mutation { liquidateMarket(symbol:"SOL-PERP") { liquidationCount events { kind } } }"#,
    )
    .await;
    assert_eq!(liq["liquidateMarket"]["liquidationCount"], 1);

    let stats = run(&schema, "{ riskStats { liquidations openPositions } }").await;
    assert_eq!(stats["riskStats"]["liquidations"], 1);
}

#[tokio::test]
async fn invalid_symbol_is_a_graphql_error() {
    let (schema, _engine) = schema_with_engine();
    let resp = schema
        .execute(Request::new(r#"{ marketState(symbol:"") { symbol } }"#))
        .await;
    assert!(!resp.errors.is_empty());
}

#[tokio::test]
async fn subscription_streams_live_events() {
    let (schema, engine) = schema_with_engine();
    let schema = Arc::new(schema);

    let mut stream = schema.execute_stream(Request::new("subscription { riskEvents { kind } }"));

    // Emit an event shortly after the subscription is registered.
    let emitter = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        engine
            .apply(marginguard_core::RiskCommand::UpdateMarket {
                symbol: marginguard_types::Symbol::new("SOL-PERP").unwrap(),
                mark_price: marginguard_types::Price::from_whole(100).unwrap(),
                index_price: marginguard_types::Price::from_whole(100).unwrap(),
                funding_rate_bps: 0,
            })
            .await
            .unwrap();
    });

    let next = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("subscription produced an event")
        .expect("stream not closed");
    emitter.await.unwrap();

    let data = next.data.into_json().unwrap();
    assert_eq!(data["riskEvents"]["kind"], "market_updated");
}
