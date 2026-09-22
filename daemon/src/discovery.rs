use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::net::TcpStream;
use tracing::{info, warn};

const SERVICE_TYPE: &str = "_hyprconnect._tcp.local.";

pub struct DiscoveryIdentity {
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub protocol_version: u32,
    pub port: u16,
}

pub async fn run(
    identity: DiscoveryIdentity,
    x25519_secret: [u8; 32],
    my_pub: [u8; 32],
) -> Result<()> {
    let mdns = ServiceDaemon::new().context("failed to start mDNS daemon")?;

    let host_name = format!("{}.local.", identity.device_id);
    let properties = [
        ("device_id", identity.device_id.as_str()),
        ("device_name", identity.device_name.as_str()),
        ("device_type", identity.device_type.as_str()),
        ("protocol_version", &identity.protocol_version.to_string()),
    ];

    let service_info = ServiceInfo::new(
        SERVICE_TYPE,
        &identity.device_id,
        &host_name,
        "",
        identity.port,
        &properties[..],
    )
    .context("failed to build mDNS service info")?
    .enable_addr_auto();

    mdns.register(service_info)
        .context("failed to register mDNS service")?;

    info!(
        "advertising as {} ({}) on port {}",
        identity.device_name, identity.device_id, identity.port
    );

    let receiver = mdns
        .browse(SERVICE_TYPE)
        .context("failed to start mDNS browse")?;

    // Tracks devices we've actually committed to dialing — NOT
    // devices we've merely seen. A "no IPv4 yet" skip must stay
    // retryable, since mDNS re-announces periodically and a later
    // event may include the address we're waiting for.
    let already_connected: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let my_device_id = identity.device_id.clone();

    loop {
        match receiver.recv_async().await {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let props = info.get_properties();
                let remote_id = props
                    .get_property_val_str("device_id")
                    .unwrap_or("unknown")
                    .to_string();

                if remote_id == my_device_id {
                    continue;
                }

                {
                    let seen = already_connected.lock().unwrap();
                    if seen.contains(&remote_id) {
                        continue;
                    }
                }

                // Symmetric discovery means both sides could try to
                // dial each other simultaneously. Break the tie
                // deterministically — only the side with the
                // "smaller" device_id dials out; the other just
                // waits to accept the incoming connection.
                if my_device_id > remote_id {
                    info!(
                        "discovered {} ({}) but yielding dial to them (tie-break)",
                        props
                            .get_property_val_str("device_name")
                            .unwrap_or("unknown"),
                        remote_id
                    );
                    continue;
                }

                let remote_name = props
                    .get_property_val_str("device_name")
                    .unwrap_or("unknown")
                    .to_string();

                // Prefer IPv4 — link-local IPv6 (fe80::...) needs a
                // network interface scope id to be connectable, which
                // we don't have here. If IPv4 hasn't resolved yet
                // (mDNS can fire ServiceResolved before all addresses
                // are known), skip WITHOUT marking this device as
                // seen, so a later announcement gets another chance.
                let addresses = info.get_addresses();
                let Some(addr) = addresses.iter().find(|a| a.is_ipv4()) else {
                    info!(
                        "{} ({}) has no IPv4 address yet, waiting for next announcement",
                        remote_name, remote_id
                    );
                    continue;
                };

                // Only now do we commit to dialing.
                {
                    let mut seen = already_connected.lock().unwrap();
                    seen.insert(remote_id.clone());
                }

                let target = format!("{}:{}", addr, info.get_port());

                info!(
                    "discovered device: {} ({}) at {} — dialing",
                    remote_name, remote_id, target
                );

                let secret = x25519_secret;
                let remote_name_clone = remote_name.clone();
                let remote_id_clone = remote_id.clone();

                tokio::spawn(async move {
                    if let Err(e) =
                        connect_and_handshake(&target, secret, my_pub, &remote_name_clone).await
                    {
                        warn!(
                            "failed to connect/handshake with {} ({}): {e:#}",
                            remote_name_clone, remote_id_clone
                        );
                    }
                });
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    Ok(())
}

async fn connect_and_handshake(
    target: &str,
    x25519_secret: [u8; 32],
    my_pub: [u8; 32],
    remote_name: &str,
) -> Result<()> {
    let mut stream = TcpStream::connect(target)
        .await
        .context("TCP connect failed")?;

    info!("connected via TCP to {}", target);

    let (transport, remote_pub) =
        crate::noise::handshake_as_initiator(&mut stream, &x25519_secret).await?;

    info!("Noise handshake succeeded with {} (as initiator)", remote_name);

    crate::session::run_session(&mut stream, transport, my_pub, remote_pub, remote_name).await
}
