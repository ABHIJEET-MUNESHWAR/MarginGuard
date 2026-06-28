//! The risk engine: applies commands, runs the liquidation waterfall, and emits
//! events. Matching/risk logic runs synchronously under a single lock so it is
//! deterministic; only persistence and publishing are async.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use marginguard_resilience::{Clock, RateLimiter, SystemClock};
use marginguard_types::{
    AccountHealth, Liquidation, LiquidationReason, MarketState, Position, RiskParams, RiskStats,
    Symbol, Usd,
};

use crate::command::RiskCommand;
use crate::config::EngineConfig;
use crate::error::CoreError;
use crate::event::RiskEvent;
use crate::insurance::InsuranceFund;
use crate::margin;
use crate::ports::{EventSink, PositionStore};

/// The result of applying a command: the ordered events it produced.
#[derive(Debug, Clone, Default)]
pub struct CommandOutcome {
    /// Events emitted, in order.
    pub events: Vec<RiskEvent>,
}

impl CommandOutcome {
    /// Number of liquidation events in the outcome.
    #[must_use]
    pub fn liquidation_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(e, RiskEvent::Liquidated(_)))
            .count()
    }
}

/// In-memory market registry and statistics, guarded by the engine lock.
struct EngineState {
    markets: HashMap<String, MarketState>,
    risk: HashMap<String, RiskParams>,
    insurance: InsuranceFund,
    stats: RiskStats,
}

/// The risk engine. Generic over the clock so the rate limiter is testable.
pub struct RiskEngine<C: Clock = SystemClock> {
    store: Arc<dyn PositionStore>,
    sink: Arc<dyn EventSink>,
    limiter: Arc<RateLimiter<C>>,
    state: Arc<Mutex<EngineState>>,
    config: EngineConfig,
}

impl<C: Clock> Clone for RiskEngine<C> {
    fn clone(&self) -> Self {
        RiskEngine {
            store: self.store.clone(),
            sink: self.sink.clone(),
            limiter: self.limiter.clone(),
            state: self.state.clone(),
            config: self.config.clone(),
        }
    }
}

impl RiskEngine<SystemClock> {
    /// Create an engine on the system clock with a seeded insurance fund.
    #[must_use]
    pub fn new(
        store: Arc<dyn PositionStore>,
        sink: Arc<dyn EventSink>,
        insurance_seed: Usd,
        config: EngineConfig,
    ) -> Self {
        let limiter = Arc::new(RateLimiter::new(
            config.ingest_capacity,
            config.ingest_refill_per_sec,
        ));
        RiskEngine::from_parts(store, sink, limiter, insurance_seed, config)
    }
}

impl<C: Clock> RiskEngine<C> {
    /// Create an engine with an explicit rate limiter (for tests).
    pub fn from_parts(
        store: Arc<dyn PositionStore>,
        sink: Arc<dyn EventSink>,
        limiter: Arc<RateLimiter<C>>,
        insurance_seed: Usd,
        config: EngineConfig,
    ) -> Self {
        let state = EngineState {
            markets: HashMap::new(),
            risk: HashMap::new(),
            insurance: InsuranceFund::new(insurance_seed),
            stats: RiskStats::default(),
        };
        RiskEngine {
            store,
            sink,
            limiter,
            state: Arc::new(Mutex::new(state)),
            config,
        }
    }

    /// Current insurance-fund balance.
    pub fn insurance_balance(&self) -> Usd {
        self.state.lock().insurance.balance()
    }

    /// A copy of the current statistics.
    pub fn stats(&self) -> RiskStats {
        self.state.lock().stats
    }

    /// Current market state, if known.
    pub fn market(&self, symbol: &Symbol) -> Option<MarketState> {
        self.state.lock().markets.get(symbol.as_str()).cloned()
    }

    /// Risk parameters for a market (falls back to the engine default).
    pub fn risk_params(&self, symbol: &Symbol) -> RiskParams {
        self.state
            .lock()
            .risk
            .get(symbol.as_str())
            .copied()
            .unwrap_or(self.config.default_risk)
    }

    /// Compute the health of a single account/market position.
    ///
    /// # Errors
    /// Returns an error if the store fails or the position/market is absent.
    pub async fn account_health(
        &self,
        account: &str,
        symbol: &Symbol,
    ) -> Result<AccountHealth, CoreError> {
        let pos = self
            .store
            .get(account, symbol)
            .await?
            .ok_or(CoreError::PositionNotFound)?;
        let mark = self.mark_for(symbol)?;
        let params = self.risk_params(symbol);
        Ok(margin::account_health(&pos, mark, &params))
    }

    /// Fetch a single position by account and market.
    ///
    /// # Errors
    /// Returns an error if the store fails.
    pub async fn position(
        &self,
        account: &str,
        symbol: &Symbol,
    ) -> Result<Option<Position>, CoreError> {
        Ok(self.store.get(account, symbol).await?)
    }

    /// All positions currently open in a market.
    ///
    /// # Errors
    /// Returns an error if the store fails.
    pub async fn positions_in(&self, symbol: &Symbol) -> Result<Vec<Position>, CoreError> {
        Ok(self.store.by_market(symbol).await?)
    }

    /// Apply a command, persist resulting positions, and publish events.
    ///
    /// # Errors
    /// Returns a [`CoreError`] on validation, throttling, or port failure.
    pub async fn apply(&self, command: RiskCommand) -> Result<CommandOutcome, CoreError> {
        if !self.limiter.try_acquire() {
            return Err(CoreError::RateLimited);
        }
        let outcome = match command {
            RiskCommand::OpenPosition { .. } => self.open(command).await?,
            RiskCommand::ClosePosition { account, symbol } => {
                self.close(account.as_str(), &symbol).await?
            }
            RiskCommand::UpdateMarket {
                symbol,
                mark_price,
                index_price,
                funding_rate_bps,
            } => self.update_market(symbol, mark_price, index_price, funding_rate_bps),
            RiskCommand::AccrueFunding { symbol } => self.accrue_funding(&symbol).await?,
            RiskCommand::LiquidateMarket { symbol } => self.liquidate_market(&symbol).await?,
        };
        if !outcome.events.is_empty() {
            self.sink.publish(&outcome.events).await?;
            self.record_metrics(&outcome.events);
        }
        Ok(outcome)
    }

    fn mark_for(&self, symbol: &Symbol) -> Result<marginguard_types::Price, CoreError> {
        self.state
            .lock()
            .markets
            .get(symbol.as_str())
            .map(|m| m.mark_price)
            .ok_or(CoreError::MarketNotFound)
    }

    async fn open(&self, command: RiskCommand) -> Result<CommandOutcome, CoreError> {
        let RiskCommand::OpenPosition {
            account,
            symbol,
            side,
            margin_mode,
            size,
            entry_price,
            leverage,
            margin: posted,
        } = command
        else {
            unreachable!("open called with non-open command");
        };

        if self.store.get(account.as_str(), &symbol).await?.is_some() {
            return Err(CoreError::PositionExists);
        }

        let position = Position {
            account: account.clone(),
            symbol: symbol.clone(),
            side,
            margin_mode,
            size,
            entry_price,
            leverage,
            posted_margin: posted,
            funding_paid: Usd::ZERO,
        };

        // Require posted margin to meet the initial-margin requirement.
        let params = self.risk_params(&symbol);
        let required = margin::initial_margin(&position, entry_price, &params);
        if posted < required {
            return Err(CoreError::InsufficientMargin);
        }

        let notional = position.notional(entry_price);
        self.store.upsert(position).await?;
        {
            let mut state = self.state.lock();
            state.stats.open_positions += 1;
        }
        Ok(CommandOutcome {
            events: vec![RiskEvent::PositionOpened {
                account,
                symbol,
                side,
                notional,
            }],
        })
    }

    async fn close(&self, account: &str, symbol: &Symbol) -> Result<CommandOutcome, CoreError> {
        let pos = self
            .store
            .remove(account, symbol)
            .await?
            .ok_or(CoreError::PositionNotFound)?;
        let mark = self.mark_for(symbol)?;
        let realized = margin::position_equity(&pos, mark).saturating_sub(pos.posted_margin);
        {
            let mut state = self.state.lock();
            state.stats.open_positions = state.stats.open_positions.saturating_sub(1);
        }
        Ok(CommandOutcome {
            events: vec![RiskEvent::PositionClosed {
                account: pos.account,
                symbol: symbol.clone(),
                realized_pnl: realized,
            }],
        })
    }

    fn update_market(
        &self,
        symbol: Symbol,
        mark_price: marginguard_types::Price,
        index_price: marginguard_types::Price,
        funding_rate_bps: i64,
    ) -> CommandOutcome {
        let mut state = self.state.lock();
        state.markets.insert(
            symbol.as_str().to_string(),
            MarketState {
                symbol: symbol.clone(),
                mark_price,
                index_price,
                funding_rate_bps,
            },
        );
        CommandOutcome {
            events: vec![RiskEvent::MarketUpdated {
                symbol,
                mark_price: mark_price.micros(),
                funding_rate_bps,
            }],
        }
    }

    async fn accrue_funding(&self, symbol: &Symbol) -> Result<CommandOutcome, CoreError> {
        let (mark, rate_bps) = {
            let state = self.state.lock();
            let m = state
                .markets
                .get(symbol.as_str())
                .ok_or(CoreError::MarketNotFound)?;
            (m.mark_price, m.funding_rate_bps)
        };
        let positions = self.store.by_market(symbol).await?;
        let mut events = Vec::new();
        for mut pos in positions {
            let payment = margin::funding_payment(&pos, mark, rate_bps);
            pos.funding_paid = pos.funding_paid.saturating_add(payment);
            let account = pos.account.clone();
            self.store.upsert(pos).await?;
            events.push(RiskEvent::FundingSettled {
                account,
                symbol: symbol.clone(),
                amount: payment,
            });
        }
        if !events.is_empty() {
            let mut state = self.state.lock();
            state.stats.funding_settlements += events.len() as u64;
        }
        Ok(CommandOutcome { events })
    }

    /// The liquidation waterfall: for every position breaching maintenance
    /// margin, close it at mark, route the liquidation fee and any solvent
    /// surplus to the insurance fund, and cover any shortfall from the fund —
    /// socialising the uncovered remainder via auto-deleveraging.
    async fn liquidate_market(&self, symbol: &Symbol) -> Result<CommandOutcome, CoreError> {
        let (mark, params) = {
            let state = self.state.lock();
            let m = state
                .markets
                .get(symbol.as_str())
                .ok_or(CoreError::MarketNotFound)?;
            (
                m.mark_price,
                state
                    .risk
                    .get(symbol.as_str())
                    .copied()
                    .unwrap_or(self.config.default_risk),
            )
        };

        let positions = self.store.by_market(symbol).await?;
        let mut events = Vec::new();
        let mut total_socialized = Usd::ZERO;

        for pos in positions {
            if !margin::is_liquidatable(&pos, mark, &params) {
                continue;
            }

            let account = pos.account.clone();
            let closed_notional = pos.notional(mark);
            let equity = margin::position_equity(&pos, mark);
            let fee = closed_notional.mul_bps(params.liquidation_fee_bps);

            // Remove the position first.
            self.store.remove(account.as_str(), symbol).await?;

            let (reason, insurance_draw, socialized) = if equity.is_negative() {
                // Bankrupt: the fund must cover the negative equity (the shortfall).
                let shortfall = -equity;
                let (drawn, soc) = {
                    let mut state = self.state.lock();
                    state.insurance.cover(shortfall)
                };
                total_socialized = total_socialized.saturating_add(soc);
                (LiquidationReason::Bankruptcy, drawn, soc)
            } else {
                // Solvent liquidation: fee (capped at remaining equity) funds the pool.
                let credited = fee.min(equity);
                {
                    let mut state = self.state.lock();
                    state.insurance.credit(credited);
                }
                (LiquidationReason::MaintenanceBreach, Usd::ZERO, Usd::ZERO)
            };

            {
                let mut state = self.state.lock();
                state.stats.open_positions = state.stats.open_positions.saturating_sub(1);
                state.stats.liquidations += 1;
            }

            events.push(RiskEvent::Liquidated(Liquidation {
                account,
                symbol: symbol.clone(),
                reason,
                closed_notional,
                insurance_draw,
                socialized_loss: socialized,
            }));
        }

        if !total_socialized.micros().eq(&0) {
            let mut state = self.state.lock();
            state.stats.adl_events += 1;
            events.push(RiskEvent::AutoDeleveraged {
                symbol: symbol.clone(),
                socialized_loss: total_socialized,
            });
        }

        Ok(CommandOutcome { events })
    }

    fn record_metrics(&self, events: &[RiskEvent]) {
        for event in events {
            match event {
                RiskEvent::PositionOpened { .. } => {
                    metrics::counter!("marginguard_positions_opened_total").increment(1);
                }
                RiskEvent::PositionClosed { .. } => {
                    metrics::counter!("marginguard_positions_closed_total").increment(1);
                }
                RiskEvent::Liquidated(l) => {
                    metrics::counter!("marginguard_liquidations_total", "reason" => l.reason.code())
                        .increment(1);
                }
                RiskEvent::FundingSettled { .. } => {
                    metrics::counter!("marginguard_funding_settlements_total").increment(1);
                }
                RiskEvent::AutoDeleveraged { .. } => {
                    metrics::counter!("marginguard_adl_events_total").increment(1);
                }
                RiskEvent::MarketUpdated { .. } => {
                    metrics::counter!("marginguard_market_updates_total").increment(1);
                }
            }
        }
    }
}

// Side is re-exported for downstream convenience.
pub use marginguard_types::Side as PositionSide;
