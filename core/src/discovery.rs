use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use once_cell::sync::OnceCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::pairing::PairingConfirmer;
use crate::protocol::{
    protocol_version_is_compatible, SERVICE_TYPE, TXT_DEVICE_ID, TXT_DEVICE_NAME, TXT_DEVICE_TYPE,
    TXT_PROTOCOL_VERSION,
};

#[derive(Clone, Debug)]
pub struct DiscoveredDevice {
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub address: Option<String>,
    pub port: u16,
}

static DISCOVERED: OnceCell<Mutex<HashMap<String, DiscoveredDevice>>> = OnceCell::new();

fn discovered_slot() -> &'static Mutex<HashMap<String, DiscoveredDevice>> {
    DISCOVERED.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn discovered_devices() -> Vec<DiscoveredDevice> {
    discovered_slot()
        .lock()
        .unwrap()
        .values()
        .cloned()
        .collect()
}

pub fn get_discovered(device_id: &str) -> Option<DiscoveredDevice> {
    discovered_slot().lock().unwrap().get(device_id).cloned()
}

fn get_discovered_by_address(address: &str) -> Option<DiscoveredDevice> {
    discovered_slot()
        .lock()
        .unwrap()
        .values()
        .find(|device| device.address.as_deref() == Some(address))
        .cloned()
}

pub async fn wait_for_discovered_by_address(address: &str) -> Option<DiscoveredDevice> {
    // A TCP connection can arrive immediately after Noise completes while the
    // corresponding mDNS ServiceResolved event is still being delivered.
    // This wait does not authenticate the peer; Noise already did that. It
    // only gives us time to obtain the peer's display and routing metadata.
    for _ in 0..50 {
        if let Some(device) = get_discovered_by_address(address) {
            return Some(device);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    None
}

fn clear_connection_marker(already_connected: &Mutex<HashSet<String>>, device_id: &str) {
    already_connected.lock().unwrap().remove(device_id);
}

pub struct DiscoveryIdentity {
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub protocol_version: u32,
    pub port: u16,
    pub auto_pair: bool,
}

pub async fn run(
    identity: DiscoveryIdentity,
    x25519_secret: [u8; 32],
    my_pub: [u8; 32],
    confirmer: Arc<dyn PairingConfirmer>,
) -> Result<()> {
    let mdns = ServiceDaemon::new().context("failed to start mDNS daemon")?;

    let host_name = format!("{}.local.", identity.device_id);
    let properties = [
        (TXT_DEVICE_ID, identity.device_id.as_str()),
        (TXT_DEVICE_NAME, identity.device_name.as_str()),
        (TXT_DEVICE_TYPE, identity.device_type.as_str()),
        (TXT_PROTOCOL_VERSION, &identity.protocol_version.to_string()),
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
                    .get_property_val_str(TXT_DEVICE_ID)
                    .unwrap_or("unknown")
                    .to_string();

                if remote_id == my_device_id {
                    continue;
                }

                let remote_name = props
                    .get_property_val_str(TXT_DEVICE_NAME)
                    .unwrap_or("unknown")
                    .to_string();
                let remote_type = props
                    .get_property_val_str(TXT_DEVICE_TYPE)
                    .unwrap_or("unknown")
                    .to_string();
                let remote_protocol_version = props.get_property_val_str(TXT_PROTOCOL_VERSION);
                if !protocol_version_is_compatible(remote_protocol_version) {
                    warn!(
                        "ignoring {} ({}) with incompatible protocol version {:?}",
                        remote_name, remote_id, remote_protocol_version
                    );
                    continue;
                }
                let address = info
                    .get_addresses()
                    .iter()
                    .find(|a| a.is_ipv4())
                    .map(ToString::to_string);
                discovered_slot().lock().unwrap().insert(
                    remote_id.clone(),
                    DiscoveredDevice {
                        device_id: remote_id.clone(),
                        device_name: remote_name.clone(),
                        device_type: remote_type.clone(),
                        address: address.clone(),
                        port: info.get_port(),
                    },
                );

                if !identity.auto_pair {
                    continue;
                }

                // Automatic reconnect is limited to devices that have
                // already completed the verification flow. New devices
                // must still be paired explicitly by the user.
                if !crate::trust::is_trusted_device_id(&remote_id) {
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

                // Prefer IPv4 — link-local IPv6 (fe80::...) needs a
                // network interface scope id to be connectable, which
                // we don't have here. If IPv4 hasn't resolved yet
                // (mDNS can fire ServiceResolved before all addresses
                // are known), skip WITHOUT marking this device as
                // seen, so a later announcement gets another chance.
                let Some(addr) = address else {
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
                let connected = already_connected.clone();
                let confirmer = confirmer.clone();

                tokio::spawn(async move {
                    let result = connect_and_handshake(
                        &target,
                        secret,
                        my_pub,
                        &remote_id_clone,
                        &remote_name_clone,
                        &remote_type,
                        confirmer,
                    )
                    .await;
                    // Clear the in-flight marker for every exit path, including
                    // a normally terminated session, so a later mDNS event can
                    // establish a fresh connection.
                    clear_connection_marker(&connected, &remote_id_clone);
                    if let Err(e) = result {
                        warn!(
                            "failed to connect/handshake with {} ({}): {e:#}",
                            remote_name_clone, remote_id_clone
                        );
                    }
                });
            }
            Ok(ServiceEvent::ServiceRemoved(_, fullname)) => {
                if let Some(device_id) = fullname.split('.').next() {
                    discovered_slot().lock().unwrap().remove(device_id);
                    already_connected.lock().unwrap().remove(device_id);
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    Ok(())
}

pub(crate) async fn connect_and_handshake(
    target: &str,
    x25519_secret: [u8; 32],
    my_pub: [u8; 32],
    remote_id: &str,
    remote_name: &str,
    remote_type: &str,
    confirmer: Arc<dyn PairingConfirmer>,
) -> Result<()> {
    let mut stream = TcpStream::connect(target)
        .await
        .context("TCP connect failed")?;

    info!("connected via TCP to {}", target);

    let (transport, remote_pub) = match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        crate::noise::handshake_as_initiator(&mut stream, &x25519_secret),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            warn!("handshake with {} timed out after 10 seconds", remote_name);
            anyhow::bail!("Noise handshake timed out after 10 seconds")
        }
    };

    info!(
        "Noise handshake succeeded with {} (as initiator)",
        remote_name
    );

    crate::session::run_session(
        stream,
        transport,
        my_pub,
        remote_pub,
        remote_id,
        remote_name,
        remote_type,
        confirmer,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::clear_connection_marker;
    use std::collections::HashSet;
    use std::sync::Mutex;

    #[test]
    fn connection_marker_is_cleared_after_attempt_ends() {
        let connected = Mutex::new(HashSet::from(["device-a".to_string()]));
        clear_connection_marker(&connected, "device-a");
        assert!(!connected.lock().unwrap().contains("device-a"));
    }
}
