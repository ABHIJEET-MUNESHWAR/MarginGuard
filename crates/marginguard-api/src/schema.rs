//! Schema assembly.

use async_graphql::{EmptySubscription, Schema};

use crate::context::ApiContext;
use crate::mutation::MutationRoot;
use crate::query::QueryRoot;
use crate::subscription::SubscriptionRoot;

/// The fully-typed MarginGuard schema.
pub type MarginGuardSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Depth/complexity guards to bound resolver work per request.
const MAX_DEPTH: usize = 12;
const MAX_COMPLEXITY: usize = 512;

/// Build the schema with the given context and safety limits.
#[must_use]
pub fn build_schema(context: ApiContext) -> MarginGuardSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .limit_depth(MAX_DEPTH)
        .limit_complexity(MAX_COMPLEXITY)
        .data(context)
        .finish()
}

/// A schema for SDL export / tooling that needs no live context.
#[must_use]
pub fn sdl() -> String {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .finish()
        .sdl()
}
