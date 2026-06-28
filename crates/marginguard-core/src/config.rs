//! Engine configuration.

use marginguard_types::RiskParams;

/// Tunable engine parameters.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Token-bucket capacity for command ingestion.
    pub ingest_capacity: f64,
    /// Token refill rate per second for command ingestion.
    pub ingest_refill_per_sec: f64,
    /// Default risk parameters applied to markets without an override.
    pub default_risk: RiskParams,
    /// Funding interval in seconds (informational; accrual is event-driven).
    pub funding_interval_secs: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            ingest_capacity: 10_000.0,
            ingest_refill_per_sec: 100_000.0,
            default_risk: RiskParams::standard(),
            funding_interval_secs: 3_600,
        }
    }
}
