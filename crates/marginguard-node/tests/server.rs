//! Integration tests for the node: HTTP routing, GraphQL over axum, the metrics
//! endpoint, and the deterministic crash simulator.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tower::ServiceExt; // for `oneshot`

use marginguard_node::config::{ServeArgs, SimulateArgs};
use marginguard_node::simulate::run_simulation;
use marginguard_node::startup::{build_advisor, build_server};

/// A Prometheus handle that is *not* installed globally, so multiple tests can
/// each build their own without the "recorder already installed" panic.
fn local_metrics() -> PrometheusHandle {
    PrometheusBuilder::new().build_recorder().handle()
}

async fn body_string(res: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}

#[tokio::test]
async fn health_endpoints_respond_ok() {
    let (app, _engine) = build_server(&ServeArgs::default(), local_metrics());
    for path in ["/health/live", "/health/ready"] {
        let res = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{path}");
    }
}

#[tokio::test]
async fn graphiql_is_served() {
    let (app, _engine) = build_server(&ServeArgs::default(), local_metrics());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/graphql")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_string(res).await;
    assert!(html.contains("GraphiQL") || html.contains("graphiql"));
}

#[tokio::test]
async fn graphql_api_version_query() {
    let (app, _engine) = build_server(&ServeArgs::default(), local_metrics());
    let query = serde_json::json!({ "query": "{ apiVersion }" }).to_string();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/graphql")
                .header("content-type", "application/json")
                .body(Body::from(query))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body_string(res).await).unwrap();
    assert!(json["data"]["apiVersion"].is_string(), "unexpected: {json}");
}

#[tokio::test]
async fn metrics_endpoint_renders() {
    let (app, _engine) = build_server(&ServeArgs::default(), local_metrics());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn simulation_produces_liquidations_on_crash() {
    let args = SimulateArgs {
        accounts: 12,
        steps: 60,
        start_price: 100,
        drift_bps: -80,
        vol_bps: 30,
        funding_bps: 10,
        funding_interval: 20,
        insurance_seed: 5_000,
        ..SimulateArgs::default()
    };
    let report = run_simulation(args).await.unwrap();
    assert_eq!(report.accounts, 12);
    assert!(report.steps == 60);
    assert!(
        report.liquidations > 0,
        "expected liquidations on a downward crash, got {}",
        report.liquidations
    );
    assert!(report.survivors <= 12);
    assert!(report.final_price_micros < report.start_price_micros);
}

#[tokio::test]
async fn heuristic_advisor_is_constructible() {
    use marginguard_node::config::Advisor;
    let advisor = build_advisor(Advisor::Heuristic);
    // Smoke: the trait object is usable.
    assert!(std::ptr::addr_of!(*advisor) as *const () as usize != 0);
}
