//! Postgres-backed [`PositionStore`] (feature `postgres`).
//!
//! Money is stored as TEXT holding the exact i128 micro-USD value — never
//! `f64` and never `NUMERIC` round-trips. The table is **hash-partitioned by
//! symbol** so a busy market's rows stay in their own partition, which keeps
//! per-market scans and index maintenance bounded as the book grows.
//!
//! Queries use the runtime (`sqlx::query`) API rather than the compile-time
//! macros, so building this crate needs no live `DATABASE_URL`.

use async_trait::async_trait;
use sqlx::{PgPool, Row};

use marginguard_core::{PortError, PositionStore};
use marginguard_types::{
    AccountId, Leverage, MarginMode, Position, Price, Side, Size, Symbol, Usd,
};

/// Number of hash partitions created by [`PgPositionStore::ensure_schema`].
const PARTITIONS: u32 = 4;

/// A Postgres position store.
#[derive(Clone)]
pub struct PgPositionStore {
    pool: PgPool,
}

impl PgPositionStore {
    /// Wrap an existing connection pool.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        PgPositionStore { pool }
    }

    /// Create the partitioned table and its partitions if absent.
    ///
    /// # Errors
    /// Returns [`PortError`] if any DDL statement fails.
    pub async fn ensure_schema(&self) -> Result<(), PortError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS positions (\
                 account TEXT NOT NULL, \
                 symbol TEXT NOT NULL, \
                 side TEXT NOT NULL, \
                 margin_mode TEXT NOT NULL, \
                 size TEXT NOT NULL, \
                 entry_price TEXT NOT NULL, \
                 leverage INT NOT NULL, \
                 posted_margin TEXT NOT NULL, \
                 funding_paid TEXT NOT NULL, \
                 PRIMARY KEY (account, symbol)\
             ) PARTITION BY HASH (symbol)",
        )
        .execute(&self.pool)
        .await
        .map_err(to_port)?;

        for r in 0..PARTITIONS {
            let ddl = format!(
                "CREATE TABLE IF NOT EXISTS positions_p{r} PARTITION OF positions \
                 FOR VALUES WITH (MODULUS {PARTITIONS}, REMAINDER {r})"
            );
            sqlx::query(&ddl)
                .execute(&self.pool)
                .await
                .map_err(to_port)?;
        }
        Ok(())
    }
}

#[async_trait]
impl PositionStore for PgPositionStore {
    async fn upsert(&self, p: Position) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO positions \
                 (account, symbol, side, margin_mode, size, entry_price, leverage, \
                  posted_margin, funding_paid) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) \
             ON CONFLICT (account, symbol) DO UPDATE SET \
                 side = EXCLUDED.side, margin_mode = EXCLUDED.margin_mode, \
                 size = EXCLUDED.size, entry_price = EXCLUDED.entry_price, \
                 leverage = EXCLUDED.leverage, posted_margin = EXCLUDED.posted_margin, \
                 funding_paid = EXCLUDED.funding_paid",
        )
        .bind(p.account.as_str())
        .bind(p.symbol.as_str())
        .bind(p.side.code())
        .bind(p.margin_mode.code())
        .bind(p.size.micros().to_string())
        .bind(p.entry_price.micros().to_string())
        .bind(i64::from(p.leverage.get()))
        .bind(p.posted_margin.micros().to_string())
        .bind(p.funding_paid.micros().to_string())
        .execute(&self.pool)
        .await
        .map_err(to_port)?;
        Ok(())
    }

    async fn get(&self, account: &str, symbol: &Symbol) -> Result<Option<Position>, PortError> {
        let row = sqlx::query("SELECT * FROM positions WHERE account = $1 AND symbol = $2")
            .bind(account)
            .bind(symbol.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(to_port)?;
        row.as_ref().map(row_to_position).transpose()
    }

    async fn remove(&self, account: &str, symbol: &Symbol) -> Result<Option<Position>, PortError> {
        let row =
            sqlx::query("DELETE FROM positions WHERE account = $1 AND symbol = $2 RETURNING *")
                .bind(account)
                .bind(symbol.as_str())
                .fetch_optional(&self.pool)
                .await
                .map_err(to_port)?;
        row.as_ref().map(row_to_position).transpose()
    }

    async fn by_market(&self, symbol: &Symbol) -> Result<Vec<Position>, PortError> {
        let rows = sqlx::query("SELECT * FROM positions WHERE symbol = $1")
            .bind(symbol.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(to_port)?;
        rows.iter().map(row_to_position).collect()
    }

    async fn count(&self) -> Result<u64, PortError> {
        let row = sqlx::query("SELECT COUNT(*) AS n FROM positions")
            .fetch_one(&self.pool)
            .await
            .map_err(to_port)?;
        let n: i64 = row.try_get("n").map_err(to_port)?;
        Ok(n.max(0) as u64)
    }
}

fn to_port(e: sqlx::Error) -> PortError {
    match e {
        sqlx::Error::PoolTimedOut => PortError::Timeout,
        sqlx::Error::PoolClosed | sqlx::Error::Io(_) => PortError::Unavailable(e.to_string()),
        other => PortError::Internal(other.to_string()),
    }
}

fn parse_micros(s: &str) -> Result<i128, PortError> {
    s.parse::<i128>()
        .map_err(|_| PortError::Internal(format!("bad money value: {s}")))
}

fn parse_side(s: &str) -> Result<Side, PortError> {
    match s {
        "long" => Ok(Side::Long),
        "short" => Ok(Side::Short),
        other => Err(PortError::Internal(format!("bad side: {other}"))),
    }
}

fn parse_margin_mode(s: &str) -> Result<MarginMode, PortError> {
    match s {
        "isolated" => Ok(MarginMode::Isolated),
        "cross" => Ok(MarginMode::Cross),
        other => Err(PortError::Internal(format!("bad margin mode: {other}"))),
    }
}

fn row_to_position(row: &sqlx::postgres::PgRow) -> Result<Position, PortError> {
    let get = |c: &str| row.try_get::<String, _>(c).map_err(to_port);
    let leverage: i32 = row.try_get("leverage").map_err(to_port)?;
    Ok(Position {
        account: AccountId::new(get("account")?).map_err(|e| PortError::Internal(e.to_string()))?,
        symbol: Symbol::new(get("symbol")?).map_err(|e| PortError::Internal(e.to_string()))?,
        side: parse_side(&get("side")?)?,
        margin_mode: parse_margin_mode(&get("margin_mode")?)?,
        size: Size::from_micros(parse_micros(&get("size")?)?)
            .map_err(|e| PortError::Internal(e.to_string()))?,
        entry_price: Price::from_micros(parse_micros(&get("entry_price")?)?)
            .map_err(|e| PortError::Internal(e.to_string()))?,
        leverage: Leverage::new(u32::try_from(leverage).unwrap_or(1))
            .map_err(|e| PortError::Internal(e.to_string()))?,
        posted_margin: Usd::from_micros(parse_micros(&get("posted_margin")?)?),
        funding_paid: Usd::from_micros(parse_micros(&get("funding_paid")?)?),
    })
}
