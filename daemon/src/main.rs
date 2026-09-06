mod discovery;
mod identity;
mod noise;

use discovery::DiscoveryIdentity;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let device_identity = identity::load_or_generate()?;

    println!("device_id: {}", device_identity.device_id);

    // Bind to port 0 — let the OS pick a free port, avoiding the
    // hardcoded-port collision problem entirely.
    let listener = TcpListener::bind("0.0.0.0:0").await?;
    let real_port = listener.local_addr()?.port();
    println!("listening on port {}", real_port);

    let discovery_identity = DiscoveryIdentity {
        device_id: device_identity.device_id.clone(),
        device_name: "Shubh's Desktop".to_string(),
        device_type: "desktop".to_string(),
        protocol_version: 1,
        port: real_port,
    };

    // Spawn discovery in the background so it doesn't block us from
    // also accepting connections below.
    tokio::spawn(discovery::run(discovery_identity));

    // Accept one incoming connection and act as Noise responder.
    tokio::spawn({
        let secret = device_identity.x25519_secret.to_bytes();
        async move {
            loop {
                if let Ok((mut stream, addr)) = listener.accept().await {
                    tracing::info!("accepted connection from {}", addr);
                    match noise::handshake_as_responder(&mut stream, &secret).await {
                        Ok(mut transport) => {
                            tracing::info!("Noise handshake succeeded (as responder)");
                            if let Ok(msg) = noise::recv_encrypted(&mut stream, &mut transport).await
                            {
                                tracing::info!(
                                    "received: {}",
                                    String::from_utf8_lossy(&msg)
                                );
                            }
                        }
                        Err(e) => tracing::warn!("handshake failed: {e:#}"),
                    }
                }
            }
        }
    });

    // Keep the process alive.
    tokio::signal::ctrl_c().await?;
    Ok(())
}
