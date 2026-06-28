//! Optional LLM-backed advisor.
//!
//! The LLM never decides risk: [`HeuristicAdvisor`] computes the deterministic
//! numbers first, and the model only supplies a natural-language narrative.
//! Any transport, timeout, or parsing failure transparently falls back to the
//! heuristic, so the network is never on the critical path of a risk decision.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::warn;

use marginguard_resilience::{retry, with_timeout, RetryPolicy};

use crate::advice::{AdviceContext, AdviceSource, RiskAdvice};
use crate::advisor::{HeuristicAdvisor, RiskAdvisor};
use crate::error::AiError;

const SYSTEM_PROMPT: &str = "You are a perpetual-futures risk assistant. Given \
margin metrics, reply with one or two concise sentences explaining the \
liquidation risk and the single most useful action. Do not invent numbers.";

/// Configuration for an OpenAI-compatible chat-completions endpoint.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Full chat-completions URL.
    pub endpoint: String,
    /// Model identifier.
    pub model: String,
    /// Bearer API key. Empty disables the backend (heuristic is used).
    pub api_key: String,
    /// Per-request deadline.
    pub timeout: Duration,
    /// Maximum attempts (>= 1).
    pub max_retries: u32,
    /// Sampling temperature.
    pub temperature: f32,
    /// Response token cap.
    pub max_tokens: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        LlmConfig {
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: String::new(),
            timeout: Duration::from_secs(4),
            max_retries: 2,
            temperature: 0.2,
            max_tokens: 160,
        }
    }
}

impl LlmConfig {
    /// Whether a usable API key is configured.
    #[must_use]
    pub fn enabled(&self) -> bool {
        !self.api_key.trim().is_empty()
    }
}

/// An advisor that narrates the deterministic heuristic with an LLM, degrading
/// to the heuristic on any failure.
pub struct LlmAdvisor {
    config: LlmConfig,
    heuristic: HeuristicAdvisor,
    client: reqwest::Client,
}

impl LlmAdvisor {
    /// Build an advisor from config. The HTTP client is created eagerly so the
    /// hot path performs no setup work.
    #[must_use]
    pub fn new(config: LlmConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .unwrap_or_default();
        LlmAdvisor {
            config,
            heuristic: HeuristicAdvisor::new(),
            client,
        }
    }

    async fn narrate(&self, prompt: &str) -> Result<String, AiError> {
        if !self.config.enabled() {
            return Err(AiError::Disabled);
        }
        let policy = RetryPolicy::new(
            self.config.max_retries.max(1),
            Duration::from_millis(100),
            Duration::from_secs(1),
        );
        retry(policy, AiError::is_retryable, || self.call_once(prompt)).await
    }

    async fn call_once(&self, prompt: &str) -> Result<String, AiError> {
        let body = ChatRequest {
            model: &self.config.model,
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: SYSTEM_PROMPT,
                },
                ChatMessage {
                    role: "user",
                    content: prompt,
                },
            ],
        };
        let request = async {
            let resp = self
                .client
                .post(&self.config.endpoint)
                .bearer_auth(&self.config.api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| AiError::Transport(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(AiError::Transport(format!("status {}", resp.status())));
            }
            let parsed: ChatResponse = resp
                .json()
                .await
                .map_err(|e| AiError::Malformed(e.to_string()))?;
            parsed
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content)
                .ok_or_else(|| AiError::Malformed("no choices returned".to_string()))
        };
        with_timeout(self.config.timeout, request)
            .await
            .map_err(|_| AiError::Timeout)?
    }
}

#[async_trait]
impl RiskAdvisor for LlmAdvisor {
    async fn assess(&self, ctx: &AdviceContext) -> RiskAdvice {
        let mut base = self.heuristic.score(ctx);
        let prompt = build_prompt(ctx, &base);
        match self.narrate(&prompt).await {
            Ok(text) => {
                let text = text.trim();
                if !text.is_empty() {
                    base.summary = text.to_string();
                    base.source = AdviceSource::Llm;
                    base.confidence = (base.confidence + 0.05).min(0.99);
                }
            }
            Err(AiError::Disabled) => {}
            Err(e) => warn!(error = %e, "llm advisor degraded to heuristic"),
        }
        base
    }
}

fn build_prompt(ctx: &AdviceContext, base: &RiskAdvice) -> String {
    format!(
        "side={side} leverage={lev}x mark={mark} entry={entry} \
         margin_ratio_bps={mr} maintenance_ratio_bps={maint} \
         distance_to_liquidation_bps={dist} funding_bps={fund} tier={tier}",
        side = ctx.side.code(),
        lev = ctx.leverage,
        mark = ctx.mark_price_micros,
        entry = ctx.entry_price_micros,
        mr = base
            .margin_ratio_bps
            .map_or_else(|| "n/a".to_string(), |v| v.to_string()),
        maint = ctx.maintenance_ratio_bps(),
        dist = base
            .liquidation_distance_bps
            .map_or_else(|| "n/a".to_string(), |v| v.to_string()),
        fund = ctx.funding_rate_bps,
        tier = base.risk_level.code(),
    )
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}
