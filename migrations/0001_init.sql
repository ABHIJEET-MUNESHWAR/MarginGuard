-- MarginGuard position store schema.
--
-- Positions are hash-partitioned by `symbol` so a busy market's rows stay in
-- their own partition, spreading writes and keeping per-market scans local.
-- All money/price/size columns are stored as exact TEXT micro-USD / micro-units
-- (signed i128 serialized as a decimal string) — never f64 or NUMERIC, so PnL,
-- margin, and funding round-trip bit-for-bit. This mirrors
-- `PgPositionStore::ensure_schema`, which also creates this idempotently.

CREATE TABLE IF NOT EXISTS positions (
    account       TEXT NOT NULL,
    symbol        TEXT NOT NULL,
    side          TEXT NOT NULL,   -- 'long' | 'short'
    margin_mode   TEXT NOT NULL,   -- 'isolated' | 'cross'
    size          TEXT NOT NULL,   -- micro-units
    entry_price   TEXT NOT NULL,   -- micro-USD
    leverage      INT  NOT NULL,   -- 1..=100
    posted_margin TEXT NOT NULL,   -- micro-USD
    funding_paid  TEXT NOT NULL,   -- micro-USD (signed)
    PRIMARY KEY (account, symbol)
) PARTITION BY HASH (symbol);

CREATE TABLE IF NOT EXISTS positions_p0 PARTITION OF positions
    FOR VALUES WITH (MODULUS 4, REMAINDER 0);
CREATE TABLE IF NOT EXISTS positions_p1 PARTITION OF positions
    FOR VALUES WITH (MODULUS 4, REMAINDER 1);
CREATE TABLE IF NOT EXISTS positions_p2 PARTITION OF positions
    FOR VALUES WITH (MODULUS 4, REMAINDER 2);
CREATE TABLE IF NOT EXISTS positions_p3 PARTITION OF positions
    FOR VALUES WITH (MODULUS 4, REMAINDER 3);

-- Secondary lookup for "all positions in a market" (liquidation sweeps).
CREATE INDEX IF NOT EXISTS positions_symbol_idx ON positions (symbol);
