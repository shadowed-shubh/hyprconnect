mod discovery;
mod packet;
mod identity;
mod noise;
mod pairing;
mod session;
mod trust;

use discovery::DiscoveryIdentity;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let device_identity = identity::load_or_generate()?;

    println!("device_id: {}", device_identity.device_id);

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

    let secret = device_identity.x25519_secret.to_bytes();
    let my_pub = device_identity.x25519_public;

    tokio::spawn(discovery::run(discovery_identity, secret, my_pub));

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, addr)) = listener.accept().await {
                tracing::info!("accepted connection from {}", addr);
                match noise::handshake_as_responder(&mut stream, &secret).await {
                    Ok((transport, remote_pub)) => {
                        tracing::info!("Noise handshake succeeded (as responder)");
                        if let Err(e) = session::run_session(
                            &mut stream,
                            transport,
                            my_pub,
                            remote_pub,
                            "peer",
                        )
                        .await
                        {
                            tracing::warn!("session error: {e:#}");
                        }
                    }
                    Err(e) => tracing::warn!("handshake failed: {e:#}"),
                }
            }
        }
    });

    tokio::signal::ctrl_c().await?;
    Ok(())
}
