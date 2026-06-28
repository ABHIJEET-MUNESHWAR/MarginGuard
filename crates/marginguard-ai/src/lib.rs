//! # marginguard-ai
//!
//! A liquidation-risk advisor with two backends behind one [`RiskAdvisor`]
//! trait:
//!
//! * [`HeuristicAdvisor`] — deterministic, dependency-free, always available.
//! * `LlmAdvisor` (feature `llm`) — narrates the heuristic's deterministic
//!   numbers with an OpenAI-compatible model and **degrades to the heuristic on
//!   any failure**, so the network is never on the critical path.

#![forbid(unsafe_code)]

pub mod advice;
pub mod advisor;
pub mod error;

#[cfg(feature = "llm")]
pub mod llm;

pub use advice::{AdviceContext, AdviceSource, RiskAdvice, RiskLevel};
pub use advisor::{HeuristicAdvisor, RiskAdvisor};
pub use error::AiError;

#[cfg(feature = "llm")]
pub use llm::{LlmAdvisor, LlmConfig};
