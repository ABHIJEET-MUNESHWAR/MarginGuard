//! Advisor tests: deterministic heuristic tiers and LLM graceful fallback.

use marginguard_ai::{AdviceContext, AdviceSource, HeuristicAdvisor, RiskAdvisor, RiskLevel};
use marginguard_types::Side;

/// Build a context whose notional is 1000 USD and maintenance ratio is 250 bps,
/// with the given margin ratio and liquidatable flag.
fn ctx(margin_ratio_bps: i64, liquidatable: bool) -> AdviceContext {
    let notional = 1_000_000_000i128; // 1000 USD
    let maintenance = 25_000_000i128; // 25 USD = 2.5% = 250 bps
    let equity = notional * i128::from(margin_ratio_bps) / 10_000;
    AdviceContext {
        side: Side::Long,
        leverage: 10,
        entry_price_micros: 100_000_000,
        mark_price_micros: 100_000_000,
        funding_rate_bps: 0,
        equity_micros: equity,
        notional_micros: notional,
        maintenance_margin_micros: maintenance,
        margin_ratio_bps: Some(margin_ratio_bps),
        liquidatable,
    }
}

fn flat_ctx() -> AdviceContext {
    AdviceContext {
        side: Side::Long,
        leverage: 5,
        entry_price_micros: 100_000_000,
        mark_price_micros: 100_000_000,
        funding_rate_bps: 0,
        equity_micros: 0,
        notional_micros: 0,
        maintenance_margin_micros: 0,
        margin_ratio_bps: None,
        liquidatable: false,
    }
}

#[test]
fn maintenance_ratio_is_derived_from_notional() {
    assert_eq!(ctx(800, false).maintenance_ratio_bps(), 250);
}

#[test]
fn distance_is_margin_minus_maintenance() {
    assert_eq!(ctx(800, false).liquidation_distance_bps(), Some(550));
    assert_eq!(flat_ctx().liquidation_distance_bps(), None);
}

#[test]
fn safe_when_buffer_is_wide() {
    let a = HeuristicAdvisor::new().score(&ctx(800, false));
    assert_eq!(a.risk_level, RiskLevel::Safe);
    assert_eq!(a.liquidation_distance_bps, Some(550));
    assert_eq!(a.source, AdviceSource::Heuristic);
    assert!(a.confidence > 0.0 && a.confidence <= 1.0);
}

#[test]
fn caution_warning_critical_tiers() {
    let h = HeuristicAdvisor::new();
    assert_eq!(h.score(&ctx(600, false)).risk_level, RiskLevel::Caution); // dist 350
    assert_eq!(h.score(&ctx(400, false)).risk_level, RiskLevel::Warning); // dist 150
    assert_eq!(h.score(&ctx(250, false)).risk_level, RiskLevel::Critical); // dist 0
}

#[test]
fn liquidatable_flag_forces_critical() {
    let a = HeuristicAdvisor::new().score(&ctx(800, true));
    assert_eq!(a.risk_level, RiskLevel::Critical);
    assert!(a.confidence >= 0.9);
}

#[test]
fn flat_account_is_safe_with_flat_summary() {
    let a = HeuristicAdvisor::new().score(&flat_ctx());
    assert_eq!(a.risk_level, RiskLevel::Safe);
    assert_eq!(a.margin_ratio_bps, None);
    assert!(a.summary.to_lowercase().contains("flat"));
}

#[test]
fn risk_level_orders_by_severity() {
    assert!(RiskLevel::Safe < RiskLevel::Caution);
    assert!(RiskLevel::Caution < RiskLevel::Warning);
    assert!(RiskLevel::Warning < RiskLevel::Critical);
}

#[tokio::test]
async fn assess_matches_score() {
    let h = HeuristicAdvisor::new();
    let c = ctx(400, false);
    assert_eq!(h.assess(&c).await, h.score(&c));
}

#[cfg(feature = "llm")]
mod llm_tests {
    use marginguard_ai::{AdviceSource, LlmAdvisor, LlmConfig, RiskAdvisor};

    #[test]
    fn default_config_is_disabled() {
        assert!(!LlmConfig::default().enabled());
    }

    #[test]
    fn config_with_key_is_enabled() {
        let cfg = LlmConfig {
            api_key: "sk-test".to_string(),
            ..LlmConfig::default()
        };
        assert!(cfg.enabled());
    }

    #[tokio::test]
    async fn disabled_backend_falls_back_to_heuristic() {
        let advisor = LlmAdvisor::new(LlmConfig::default());
        let advice = advisor.assess(&super::ctx(400, false)).await;
        // No API key -> heuristic numbers, heuristic source.
        assert_eq!(advice.source, AdviceSource::Heuristic);
        assert_eq!(advice.margin_ratio_bps, Some(400));
    }

    #[tokio::test]
    async fn unreachable_endpoint_falls_back_to_heuristic() {
        // A bogus key + unroutable endpoint must still yield deterministic advice.
        let cfg = LlmConfig {
            api_key: "sk-test".to_string(),
            endpoint: "http://127.0.0.1:1/v1/chat/completions".to_string(),
            timeout: std::time::Duration::from_millis(150),
            max_retries: 1,
            ..LlmConfig::default()
        };
        let advisor = LlmAdvisor::new(cfg);
        let advice = advisor.assess(&super::ctx(250, true)).await;
        assert_eq!(advice.source, AdviceSource::Heuristic);
        assert_eq!(advice.risk_level, marginguard_ai::RiskLevel::Critical);
    }
}
