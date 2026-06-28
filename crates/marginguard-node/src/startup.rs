//! Composition root: wires adapters into the engine, builds the advisor, and
//! serves GraphQL over axum.

use std::sync::Arc;

use anyhow::Context as _;
use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use metrics_exporter_prometheus::PrometheusHandle;
use tokio::net::TcpListener;

use marginguard_ai::{HeuristicAdvisor, RiskAdvisor};
use marginguard_api::{build_schema, ApiContext, MarginGuardSchema};
use marginguard_core::{EngineConfig, RiskEngine};
use marginguard_infra::{BroadcastEventSink, MemoryPositionStore};
use marginguard_types::Usd;

use crate::config::{Advisor, ServeArgs};

/// Build the configured advisor. The LLM backend is only available with the
/// `llm` feature; otherwise the request degrades to the heuristic with a warning.
#[must_use]
pub fn build_advisor(advisor: Advisor) -> Arc<dyn RiskAdvisor> {
    match advisor {
        Advisor::Heuristic => Arc::new(HeuristicAdvisor::new()),
        Advisor::Llm => build_llm_advisor(),
    }
}

#[cfg(feature = "llm")]
fn build_llm_advisor() -> Arc<dyn RiskAdvisor> {
    use marginguard_ai::{LlmAdvisor, LlmConfig};
    let mut config = LlmConfig::default();
    if let Ok(key) = std::env::var("MARGINGUARD_LLM_API_KEY") {
        config.api_key = key;
    }
    if let Ok(endpoint) = std::env::var("MARGINGUARD_LLM_ENDPOINT") {
        config.endpoint = endpoint;
    }
    if let Ok(model) = std::env::var("MARGINGUARD_LLM_MODEL") {
        config.model = model;
    }
    if !config.enabled() {
        tracing::warn!(
            "llm advisor selected but MARGINGUARD_LLM_API_KEY is unset; using heuristic fallback"
        );
    }
    Arc::new(LlmAdvisor::new(config))
}

#[cfg(not(feature = "llm"))]
fn build_llm_advisor() -> Arc<dyn RiskAdvisor> {
    tracing::warn!("llm advisor requested but the `llm` feature is disabled; using heuristic");
    Arc::new(HeuristicAdvisor::new())
}

/// Build a fully-wired engine plus the broadcast bus shared as both the
/// write-side `EventSink` and the read-side `RiskEventStream`.
#[must_use]
pub fn build_engine(
    event_capacity: usize,
    insurance_seed: u64,
) -> (RiskEngine, Arc<BroadcastEventSink>) {
    let bus = Arc::new(BroadcastEventSink::new(event_capacity));
    let store = Arc::new(MemoryPositionStore::new());
    let engine = RiskEngine::new(
        store,
        bus.clone(),
        Usd::from_whole(i64::try_from(insurance_seed).unwrap_or(i64::MAX)),
        EngineConfig::default(),
    );
    (engine, bus)
}

/// Assemble the GraphQL context for the given serve arguments.
#[must_use]
pub fn build_context(args: &ServeArgs) -> (ApiContext, RiskEngine) {
    let (engine, bus) = build_engine(args.event_capacity, args.insurance_seed);
    let advisor = build_advisor(args.advisor);
    let context = ApiContext::new(engine.clone(), advisor, bus);
    (context, engine)
}

/// Shared HTTP state.
#[derive(Clone)]
pub struct AppState {
    schema: MarginGuardSchema,
    metrics: Arc<PrometheusHandle>,
}

/// Build the axum router exposing GraphQL, health, and metrics endpoints.
pub fn build_app(schema: MarginGuardSchema, metrics: PrometheusHandle) -> Router {
    let state = AppState {
        schema,
        metrics: Arc::new(metrics),
    };
    Router::new()
        .route("/graphql", get(graphiql).post(graphql_handler))
        .route_service(
            "/graphql/ws",
            GraphQLSubscription::new(state.schema.clone()),
        )
        .route("/health/live", get(|| async { "ok" }))
        .route("/health/ready", get(|| async { "ready" }))
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}

async fn graphql_handler(State(state): State<AppState>, req: GraphQLRequest) -> GraphQLResponse {
    state.schema.execute(req.into_inner()).await.into()
}

async fn graphiql() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql/ws")
            .finish(),
    )
}

async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    state.metrics.render()
}

/// Build the server pieces from `serve` arguments.
pub fn build_server(args: &ServeArgs, metrics: PrometheusHandle) -> (Router, RiskEngine) {
    let (context, engine) = build_context(args);
    let schema = build_schema(context);
    (build_app(schema, metrics), engine)
}

/// Run the HTTP server until a shutdown signal is received.
///
/// # Errors
/// Returns an error if the listener cannot bind or the server fails.
pub async fn run_server(args: ServeArgs, metrics: PrometheusHandle) -> anyhow::Result<()> {
    let addr = format!("{}:{}", args.host, args.port);
    let (app, _engine) = build_server(&args, metrics);
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    tracing::info!(%addr, "marginguard listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl_c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
