//! CLI configuration for the MarginGuard node.

use clap::{Args, Parser, Subcommand, ValueEnum};

/// MarginGuard — a perpetual-futures margin, funding, and liquidation engine.
#[derive(Debug, Parser)]
#[command(name = "marginguard", version, about)]
pub struct Cli {
    /// Emit logs as JSON (recommended in production).
    #[arg(long, global = true, env = "MARGINGUARD_LOG_JSON")]
    pub log_json: bool,

    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the GraphQL/HTTP server.
    Serve(ServeArgs),
    /// Run a deterministic market scenario and print a risk report.
    Simulate(SimulateArgs),
}

/// Which liquidation-risk advisor to use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Advisor {
    /// Deterministic local heuristic (no network).
    #[default]
    Heuristic,
    /// LLM-backed narration with heuristic fallback (feature `llm`).
    Llm,
}

/// Arguments for the `serve` subcommand.
#[derive(Debug, Args, Clone)]
pub struct ServeArgs {
    /// Host/IP to bind.
    #[arg(long, env = "MARGINGUARD_HOST", default_value = "127.0.0.1")]
    pub host: String,
    /// TCP port to bind.
    #[arg(long, env = "MARGINGUARD_PORT", default_value_t = 8080)]
    pub port: u16,
    /// Event broadcast channel capacity.
    #[arg(long, default_value_t = 4096)]
    pub event_capacity: usize,
    /// Insurance-fund seed in whole USD.
    #[arg(long, default_value_t = 1_000_000)]
    pub insurance_seed: u64,
    /// Risk advisor backend.
    #[arg(long, value_enum, default_value_t = Advisor::default())]
    pub advisor: Advisor,
}

impl Default for ServeArgs {
    fn default() -> Self {
        ServeArgs {
            host: "127.0.0.1".to_string(),
            port: 8080,
            event_capacity: 4096,
            insurance_seed: 1_000_000,
            advisor: Advisor::Heuristic,
        }
    }
}

/// Arguments for the `simulate` subcommand.
#[derive(Debug, Args, Clone)]
pub struct SimulateArgs {
    /// Number of accounts to open (alternating long/short).
    #[arg(long, default_value_t = 20)]
    pub accounts: u64,
    /// Number of price steps to walk.
    #[arg(long, default_value_t = 200)]
    pub steps: u64,
    /// Starting mark price in whole USD.
    #[arg(long, default_value_t = 100)]
    pub start_price: i64,
    /// Per-step drift in basis points (negative = bearish).
    #[arg(long, default_value_t = -50)]
    pub drift_bps: i64,
    /// Per-step volatility in basis points.
    #[arg(long, default_value_t = 40)]
    pub vol_bps: i64,
    /// Funding rate in basis points applied each funding interval.
    #[arg(long, default_value_t = 10)]
    pub funding_bps: i64,
    /// Steps between funding settlements.
    #[arg(long, default_value_t = 25)]
    pub funding_interval: u64,
    /// Insurance-fund seed in whole USD.
    #[arg(long, default_value_t = 5_000)]
    pub insurance_seed: u64,
    /// Deterministic seed.
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    /// Risk advisor backend used for the sample advice line.
    #[arg(long, value_enum, default_value_t = Advisor::default())]
    pub advisor: Advisor,
}

impl Default for SimulateArgs {
    fn default() -> Self {
        SimulateArgs {
            accounts: 20,
            steps: 200,
            start_price: 100,
            drift_bps: -50,
            vol_bps: 40,
            funding_bps: 10,
            funding_interval: 25,
            insurance_seed: 5_000,
            seed: 42,
            advisor: Advisor::Heuristic,
        }
    }
}
