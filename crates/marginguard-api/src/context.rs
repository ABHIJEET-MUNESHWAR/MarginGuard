//! The shared GraphQL context: the engine plus the advisor and event stream.

use std::sync::Arc;

use marginguard_ai::RiskAdvisor;
use marginguard_core::{RiskEngine, RiskEventStream};

/// Dependencies every resolver shares. Cheap to clone (all `Arc`s inside).
#[derive(Clone)]
pub struct ApiContext {
    /// The risk engine (write + read model).
    pub engine: RiskEngine,
    /// The liquidation-risk advisor (heuristic or LLM-backed).
    pub advisor: Arc<dyn RiskAdvisor>,
    /// The live event stream for subscriptions.
    pub events: Arc<dyn RiskEventStream>,
}

impl ApiContext {
    /// Assemble a context from its parts.
    #[must_use]
    pub fn new(
        engine: RiskEngine,
        advisor: Arc<dyn RiskAdvisor>,
        events: Arc<dyn RiskEventStream>,
    ) -> Self {
        ApiContext {
            engine,
            advisor,
            events,
        }
    }
}
