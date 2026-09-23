use std::sync::Arc;

use hyprconnect_core::pairing::{PairingConfirmer, StdinConfirmer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let confirmer: Arc<dyn PairingConfirmer> = Arc::new(StdinConfirmer);
    hyprconnect_core::start_discovery_with_options("Shubh's Desktop", "desktop", true, confirmer)
        .await?;

    tokio::signal::ctrl_c().await?;
    Ok(())
}
