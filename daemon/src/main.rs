use std::env;
use std::sync::Arc;

use hyprconnect_core::pairing::{AcceptAllConfirmer, PairingConfirmer, StdinConfirmer};
use hyprconnect_core::settings::{Confirmation, Settings};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--list-trusted") => {
            for device in hyprconnect_core::trust::trusted_devices()? {
                println!(
                    "{}\t{}\t{}",
                    device.device_id, device.device_name, device.device_type
                );
            }
            return Ok(());
        }
        Some("--remove-trusted") => {
            let device_id = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("usage: daemon --remove-trusted <device_id>"))?;
            let removed = hyprconnect_core::trust::remove_trusted(device_id)?;
            if removed {
                println!("removed trusted device {device_id}");
            } else {
                println!("no trusted device found with id {device_id}");
            }
            return Ok(());
        }
        Some("--help") | Some("-h") => {
            println!("usage:");
            println!("  daemon                         start discovery daemon");
            println!("  daemon --list-trusted          list trusted devices");
            println!("  daemon --remove-trusted <id>   remove a trusted device");
            return Ok(());
        }
        _ => {}
    }

    let settings = Settings::load()?;
    let confirmer: Arc<dyn PairingConfirmer> = match settings.confirmation {
        Confirmation::Prompt => Arc::new(StdinConfirmer),
        Confirmation::Auto => Arc::new(AcceptAllConfirmer),
    };
    hyprconnect_core::start_discovery_with_settings(settings, confirmer).await?;

    tokio::signal::ctrl_c().await?;
    Ok(())
}
