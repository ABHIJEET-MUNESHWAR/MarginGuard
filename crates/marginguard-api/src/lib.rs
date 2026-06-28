//! # marginguard-api
//!
//! The GraphQL surface over the risk engine. Queries expose the read model
//! (positions, health, market state, insurance fund, stats, AI advice),
//! mutations drive the engine (open/close, update market, accrue funding,
//! liquidate), and subscriptions stream live risk and liquidation events.
//!
//! Resolvers translate through anti-corruption [`dto`]s so domain invariants
//! never leak onto the wire, and exact money crosses as micro-unit strings.

#![forbid(unsafe_code)]

pub mod context;
pub mod dto;
pub mod error;
pub mod mutation;
pub mod query;
pub mod schema;
pub mod subscription;

pub use context::ApiContext;
pub use mutation::{MarginModeInput, MutationRoot, OpenPositionInput, SideInput};
pub use query::QueryRoot;
pub use schema::{build_schema, sdl, MarginGuardSchema};
pub use subscription::SubscriptionRoot;
