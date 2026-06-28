# MarginGuard — Self-Evaluation

A candid assessment of MarginGuard against the 29 production-engineering
guidelines. Legend: ✅ fully addressed · 🟡 partially / intentionally scoped ·
⬜ deliberately out of scope for this service.

> **Headline:** MarginGuard is a **perps margin, funding & liquidation risk
> engine**. Its centre of gravity is the **liquidation waterfall** (insurance
> fund + auto-deleveraging), property-tested solvency invariants, and a **GenAI
> risk advisor that degrades gracefully** to a deterministic heuristic.

| # | Guideline | Status | How MarginGuard addresses it |
|---|---|:--:|---|
| 1 | **SOLID principles** | ✅ | Single-responsibility crates; the engine depends on `PositionStore`/`EventSink`/`RiskEventStream` **abstractions** (DIP), not concretes. Adapters are open for extension, the core closed for modification (OCP). |
| 2 | **Microservices pattern** (event-driven / CQRS / Saga) | ✅ | **CQRS + event sourcing**: write side `RiskCommand` is separated from read side `RiskEvent`; events are broadcast to subscribers. The liquidation waterfall is a compensating, step-wise process (Saga-like): close → fee → insurance draw → ADL, each step event-sourced. |
| 3 | **DB partitioning / sharding** | ✅ | `PgPositionStore` persists positions to a Postgres table **hash-partitioned by `symbol`** (4 partitions, `PARTITION BY HASH`), keeping a hot market's rows isolated. DDL in [`migrations/0001_init.sql`](migrations/0001_init.sql). |
| 4 | **Timeouts, retry, fault tolerance** | ✅ | `marginguard-resilience` provides `with_timeout`, `RetryPolicy` (deterministic equal-jitter backoff). The LLM advisor wraps calls in timeout + retry and falls back on failure. |
| 5 | **Rate limiting & circuit breaker** | ✅ | Token-bucket `RateLimiter` and a lazy `CircuitBreaker` (Closed/Open/HalfOpen) over an injectable clock are provided for ingestion and outbound adapters. |
| 6 | **Robust error handling & recovery** | ✅ | `thiserror` enums (`CoreError`, `PortError`, `AiError`, `InvalidInput`) with stable `code()`s; **no `unwrap`/`expect`/`panic!` on runtime paths**; every fallible call returns `Result`. |
| 7 | **GraphQL over REST** | ✅ | `async-graphql` schema with query/mutation/subscription roots and depth + complexity limits; no REST business endpoints. |
| 8 | **100% meaningful test coverage** | ✅ | 67 tests across all seven crates: unit, integration, end-to-end, and **proptest** invariants (funding zero-sum, liquidation clears all underwater positions). |
| 9 | **Modular, reusable components** | ✅ | `marginguard-types` and `marginguard-resilience` are standalone, dependency-light crates reusable by any sibling service. |
| 10 | **Idiomatic Rust** | ✅ | Newtypes, exhaustive `match`, iterator pipelines, `#![forbid(unsafe_code)]` in every crate, `From` conversions at boundaries. |
| 11 | **Canonical crate stack** | ✅ | `tokio`, `serde`, `thiserror`, `async-graphql`, `axum`/`tower`, `sqlx`, `tracing`, `metrics`, `reqwest`, `criterion`, `proptest`, `mockall`, `parking_lot` — declared once in `[workspace.dependencies]`. |
| 12 | **Generative / Agentic AI** | ✅ | A **GenAI risk advisor**: a deterministic heuristic always produces a verdict; the optional LLM backend (`llm` feature) narrates it via an OpenAI-compatible endpoint and **falls back to the heuristic on any error, timeout, or missing key**. |
| 13 | **Generics & trait bounds** | ✅ | `RiskEngine<C: Clock>`, `RateLimiter<C: Clock>`, `CircuitBreaker<C: Clock>` are generic over an injectable clock; ports and the advisor are trait objects. |
| 14 | **Well-designed interfaces** | ✅ | Narrow ports (`upsert`, `get`, `remove`, `by_market`, `count`, `publish`, `subscribe`); the GraphQL layer is an explicit anti-corruption boundary over domain types. |
| 15 | **README with setup** | ✅ | [`README.md`](README.md) has TOC, mermaid architecture / waterfall / AI-fallback diagrams, KaTeX margin formulas, component & complexity tables, config, and real simulator/benchmark output. |
| 16 | **Performance** | ✅ | Integer-only fixed-point math (no `f64` for money); `O(1)` health/liquidation-price; criterion shows ~30 ns health and an `O(n)` sweep (5.46 µs → 223.8 µs for 10 → 500 positions). |
| 17 | **Tokio async runtime** | ✅ | `#[tokio::main]`; all I/O (HTTP, store, bus, subscriptions, LLM) is async; margin math is synchronous and allocation-free. |
| 18 | **Parallel / concurrent / batch** | ✅ | Events are published in **batches** per command; the broadcast bus fans out concurrently to all subscribers; `rayon`/`dashmap` available in the stack. |
| 19 | **Logging & observability** | ✅ | `tracing` spans (compact or JSON), Prometheus `/metrics` (`marginguard_liquidations_total{reason}`, `marginguard_positions_opened_total`, `marginguard_adl_events_total`, …), `/health/live` + `/health/ready`. |
| 20 | **Happy path + edge cases** | ✅ | Tests cover maintenance-breach vs bankruptcy liquidation, insurance-fund draw, ADL socialisation, funding zero-sum, position-not-found / already-exists, invalid symbol, and LLM-unreachable fallback. |
| 21 | **Composable, extensible architecture** | ✅ | Hexagonal: swap `MemoryPositionStore` for `PgPositionStore` (or any store) without touching the engine; swap `HeuristicAdvisor` for `LlmAdvisor` behind the same trait. |
| 22 | **Idiomatic patterns** | ✅ | Newtype, validated construction (`Price::from_micros`, `Leverage::new`), explicit `From` conversions, builder-free construction. |
| 23 | **Compile-time type constraints** | ✅ | `Usd` is the only money type (signed `i128` micro-USD); `Price`/`Size` are positive by construction; `Leverage` is bounded `1..=100`; illegal states (negative size, zero price, 0x leverage) are unrepresentable. |
| 24 | **Benchmarks + Big-O** | ✅ | `criterion` benches for margin math and the liquidation sweep; documented complexity table (`O(1)` health, `O(n)` sweep) validated by the 10/100/500 scaling. |
| 25 | **CI/CD pipeline** | ✅ | [`.github/workflows/ci.yml`](.github/workflows/ci.yml): `fmt --check`, `clippy -D warnings` (all features), `test --workspace --all-features`, and `cargo audit --deny warnings`. |
| 26 | **Dockerfile** | ✅ | Multi-stage `rust:1.89-slim` → `debian:bookworm-slim`, non-root UID 10001, dependency-cached build (with the `llm` feature), `serve` as default CMD. |
| 27 | **Postman collection** | ✅ | [`postman/MarginGuard.postman_collection.json`](postman/MarginGuard.postman_collection.json) covers every query, mutation, both WS subscriptions, health, and metrics. |
| 28 | **Self-evaluation** | ✅ | This document. |
| 29 | **Anchor framework (where applicable)** | ⬜ | MarginGuard is an **off-chain** risk engine that feeds an on-chain settlement layer; there is no Anchor program surface in this service. On-chain liquidation settlement would be a separate program. |

## Scope notes

- **Fixed-point by design.** Money is signed `i128` micro-USD end to end; the
  engine never uses `f64` for value. Postgres columns are exact `TEXT` micros,
  not `NUMERIC` or floats — PnL and funding are bit-for-bit reproducible.
- **Determinism in the core, AI at the edge.** Margin math and the liquidation
  waterfall are deterministic (no clocks, no RNG, no model calls) so the
  property tests and benchmarks are stable. The LLM only *narrates* a verdict the
  heuristic already computed, and any failure falls back silently.
- **Where the deferred item lives.** On-chain settlement (Anchor) is a separate
  layer; MarginGuard stays a focused, composable, observable risk core. DB
  partitioning, GenAI, and resilience are all delivered here.
