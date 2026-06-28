//! MarginGuard node: composition root exposing config, telemetry, startup, and
//! the simulator as a library so they can be integration-tested.

#![forbid(unsafe_code)]

pub mod config;
pub mod simulate;
pub mod startup;
pub mod telemetry;

use anyhow::Context as _;
use clap::Parser;

use crate::config::{Cli, Command};

/// Parse the CLI and dispatch to the selected subcommand.
///
/// # Errors
/// Propagates any error from telemetry setup, the server, or the simulator.
pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    telemetry::init_tracing(cli.log_json);

    match cli.command {
        Command::Serve(args) => {
            let metrics = telemetry::install_recorder().context("install metrics recorder")?;
            startup::run_server(args, metrics).await
        }
        Command::Simulate(args) => {
            let report = simulate::run_simulation(args).await?;
            simulate::print_report(&report);
            Ok(())
        }
    }
}
