//! Tracing and Prometheus metrics initialisation.

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialise structured tracing. JSON output when `json` is true.
pub fn init_tracing(json: bool) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,marginguard=debug"));
    let registry = tracing_subscriber::registry().with(filter);
    if json {
        registry
            .with(fmt::layer().json().flatten_event(true))
            .init();
    } else {
        registry.with(fmt::layer().compact()).init();
    }
}

/// Build a Prometheus recorder and install it as the global metrics sink,
/// returning a handle that renders the exposition text.
///
/// # Errors
/// Returns an error if a global recorder is already installed.
pub fn install_recorder() -> Result<PrometheusHandle, anyhow::Error> {
    let handle = PrometheusBuilder::new().install_recorder()?;
    Ok(handle)
}
