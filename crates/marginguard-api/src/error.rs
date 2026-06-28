//! Error mapping helpers for resolvers.

use std::fmt::Display;

/// Convert any displayable error into an `async_graphql::Error`.
///
/// `async-graphql` has no blanket `From<impl Display>`, so resolvers funnel
/// engine and validation errors through this helper with `.map_err(to_err)`.
pub fn to_err(e: impl Display) -> async_graphql::Error {
    async_graphql::Error::new(e.to_string())
}
