//! # marginguard-core
//!
//! The MarginGuard risk engine. Pure, deterministic margin mathematics
//! ([`margin`]) drive an event-sourced [`RiskEngine`] that opens and closes
//! positions, accrues funding, and runs a **liquidation waterfall**: solvent
//! liquidations fund an [`InsuranceFund`], bankruptcies draw from it, and any
//! uncovered shortfall is socialised via auto-deleveraging.
//!
//! The engine depends only on abstract ports ([`PositionStore`], [`EventSink`],
//! [`RiskEventStream`]) — it has no knowledge of databases or transports.

#![forbid(unsafe_code)]

pub mod command;
pub mod config;
pub mod engine;
pub mod error;
pub mod event;
pub mod insurance;
pub mod margin;
pub mod ports;

pub use command::RiskCommand;
pub use config::EngineConfig;
pub use engine::{CommandOutcome, RiskEngine};
pub use error::{CoreError, PortError};
pub use event::RiskEvent;
pub use insurance::InsuranceFund;
pub use ports::{EventSink, PositionStore, RiskEventStream};
