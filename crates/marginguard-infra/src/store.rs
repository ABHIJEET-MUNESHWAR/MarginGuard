//! In-memory [`PositionStore`] with a secondary by-market index.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use parking_lot::RwLock;

use marginguard_core::{PortError, PositionStore};
use marginguard_types::{Position, Symbol};

#[derive(Default)]
struct Inner {
    by_key: HashMap<(String, Symbol), Position>,
    by_market: HashMap<Symbol, HashSet<String>>,
}

/// A thread-safe in-memory position store. Reads take a shared lock; writes
/// take an exclusive lock. A secondary index keeps `by_market` proportional to
/// the number of positions in that market rather than the whole book.
#[derive(Default)]
pub struct MemoryPositionStore {
    inner: RwLock<Inner>,
}

impl MemoryPositionStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PositionStore for MemoryPositionStore {
    async fn upsert(&self, position: Position) -> Result<(), PortError> {
        let mut g = self.inner.write();
        let account = position.account.as_str().to_string();
        g.by_market
            .entry(position.symbol.clone())
            .or_default()
            .insert(account.clone());
        g.by_key
            .insert((account, position.symbol.clone()), position);
        Ok(())
    }

    async fn get(&self, account: &str, symbol: &Symbol) -> Result<Option<Position>, PortError> {
        let g = self.inner.read();
        Ok(g.by_key
            .get(&(account.to_string(), symbol.clone()))
            .cloned())
    }

    async fn remove(&self, account: &str, symbol: &Symbol) -> Result<Option<Position>, PortError> {
        let mut g = self.inner.write();
        let removed = g.by_key.remove(&(account.to_string(), symbol.clone()));
        if removed.is_some() {
            let empty = if let Some(set) = g.by_market.get_mut(symbol) {
                set.remove(account);
                set.is_empty()
            } else {
                false
            };
            if empty {
                g.by_market.remove(symbol);
            }
        }
        Ok(removed)
    }

    async fn by_market(&self, symbol: &Symbol) -> Result<Vec<Position>, PortError> {
        let g = self.inner.read();
        let Some(accounts) = g.by_market.get(symbol) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::with_capacity(accounts.len());
        for account in accounts {
            if let Some(p) = g.by_key.get(&(account.clone(), symbol.clone())) {
                out.push(p.clone());
            }
        }
        Ok(out)
    }

    async fn count(&self) -> Result<u64, PortError> {
        Ok(self.inner.read().by_key.len() as u64)
    }
}
