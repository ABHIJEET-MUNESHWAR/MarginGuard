//! MarginGuard binary entrypoint.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present; ignore if absent.
    let _ = dotenvy::dotenv();
    marginguard_node::run().await
}
