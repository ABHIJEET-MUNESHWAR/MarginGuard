# ---- Builder ----------------------------------------------------------------
FROM rust:1.89-slim-bookworm AS builder

WORKDIR /app

# Pre-cache dependency builds.
COPY Cargo.toml Cargo.lock ./
COPY crates/marginguard-types/Cargo.toml crates/marginguard-types/
COPY crates/marginguard-resilience/Cargo.toml crates/marginguard-resilience/
COPY crates/marginguard-core/Cargo.toml crates/marginguard-core/
COPY crates/marginguard-ai/Cargo.toml crates/marginguard-ai/
COPY crates/marginguard-infra/Cargo.toml crates/marginguard-infra/
COPY crates/marginguard-api/Cargo.toml crates/marginguard-api/
COPY crates/marginguard-node/Cargo.toml crates/marginguard-node/

# Create stub sources so the dependency graph resolves and caches.
RUN set -eux; \
    for c in types resilience core ai infra api; do \
      mkdir -p "crates/marginguard-$c/src"; \
      echo "" > "crates/marginguard-$c/src/lib.rs"; \
    done; \
    mkdir -p crates/marginguard-node/src crates/marginguard-node/benches; \
    echo "fn main() {}" > crates/marginguard-node/src/main.rs; \
    echo "" > crates/marginguard-node/src/lib.rs; \
    echo "fn main() {}" > crates/marginguard-node/benches/risk_bench.rs; \
    cargo build --release --bin marginguard --features llm || true

# Build the real sources.
COPY crates ./crates
RUN set -eux; \
    find crates -name '*.rs' -exec touch {} +; \
    cargo build --release --bin marginguard --features llm

# ---- Runtime ----------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates; \
    rm -rf /var/lib/apt/lists/*; \
    useradd --uid 10001 --user-group --no-create-home --shell /usr/sbin/nologin marginguard

COPY --from=builder /app/target/release/marginguard /usr/local/bin/marginguard

USER 10001
EXPOSE 8080
ENV MARGINGUARD_LOG_JSON=true

ENTRYPOINT ["/usr/local/bin/marginguard"]
CMD ["serve", "--host", "0.0.0.0", "--port", "8080"]
