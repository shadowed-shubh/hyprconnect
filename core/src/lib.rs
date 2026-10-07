pub mod discovery;
pub mod features;
pub mod identity;
pub mod noise;
pub mod packet;
pub mod pairing;
pub mod session;
pub mod trust;

use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpListener;
use tracing::warn;

#[derive(Debug, Error)]
pub enum HyprConnectError {
    #[error("device not found")]
    DeviceNotFound,
    #[error("not paired with this device")]
    NotPaired,
    #[error("another pairing is already in progress")]
    PairingInProgress,
    #[error("transport error: {0}")]
    Transport(String),
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub trusted: bool,
}

pub trait DeviceEventCallback: Send + Sync {
    fn on_pairing_code(&self, device_id: String, code: String);
}

pub async fn pair(device_id: String) -> Result<(), HyprConnectError> {
    if pairing::is_pairing_in_progress() {
        return Err(HyprConnectError::PairingInProgress);
    }
    let device = discovery::get_discovered(&device_id).ok_or(HyprConnectError::DeviceNotFound)?;
    let address = device.address.ok_or_else(|| {
        HyprConnectError::Transport("device has no resolved IPv4 address".to_string())
    })?;
    let target = format!("{}:{}", address, device.port);
    let identity =
        identity::load_or_generate().map_err(|e| HyprConnectError::Transport(e.to_string()))?;
    let confirmer: std::sync::Arc<dyn pairing::PairingConfirmer> =
        std::sync::Arc::new(pairing::CallbackConfirmer);

    discovery::connect_and_handshake(
        &target,
        identity.x25519_secret.to_bytes(),
        identity.x25519_public,
        &device.device_id,
        &device.device_name,
        &device.device_type,
        confirmer,
    )
    .await
    .map_err(|e| HyprConnectError::Transport(e.to_string()))
}

pub fn list_devices() -> Vec<DeviceInfo> {
    let trusted = trust::trusted_devices().unwrap_or_default();
    let mut devices = std::collections::HashMap::new();

    for device in trusted {
        devices.insert(
            device.device_id.clone(),
            DeviceInfo {
                device_id: device.device_id,
                device_name: device.device_name,
                device_type: device.device_type,
                trusted: true,
            },
        );
    }

    for device in discovery::discovered_devices() {
        devices
            .entry(device.device_id.clone())
            .or_insert(DeviceInfo {
                device_id: device.device_id,
                device_name: device.device_name,
                device_type: device.device_type,
                trusted: false,
            });
    }

    devices.into_values().collect()
}

pub fn register_callback(callback: Box<dyn DeviceEventCallback>) {
    pairing::register_callback(callback);
}

pub fn confirm_pairing(device_id: String, accepted: bool) {
    pairing::confirm_pairing(device_id, accepted);
}

pub async fn start_discovery() {
    let confirmer: Arc<dyn pairing::PairingConfirmer> = Arc::new(pairing::CallbackConfirmer);
    if let Err(error) =
        start_discovery_with_options("HyprConnect Device", "phone", false, confirmer).await
    {
        warn!("failed to start discovery: {error:#}");
    }
}

pub async fn start_discovery_with_options(
    device_name: &str,
    device_type: &str,
    auto_pair: bool,
    confirmer: Arc<dyn pairing::PairingConfirmer>,
) -> anyhow::Result<()> {
    let device_identity = identity::load_or_generate()?;
    let listener = TcpListener::bind("0.0.0.0:0").await?;
    let real_port = listener.local_addr()?.port();
    let secret = device_identity.x25519_secret.to_bytes();
    let my_pub = device_identity.x25519_public;

    println!("device_id: {}", device_identity.device_id);
    println!("listening on port {}", real_port);

    let discovery_identity = discovery::DiscoveryIdentity {
        device_id: device_identity.device_id,
        device_name: device_name.to_string(),
        device_type: device_type.to_string(),
        protocol_version: 1,
        port: real_port,
        auto_pair,
    };

    tokio::spawn(discovery::run(
        discovery_identity,
        secret,
        my_pub,
        confirmer.clone(),
    ));

    tokio::spawn(async move {
        loop {
            let Ok((mut stream, addr)) = listener.accept().await else {
                continue;
            };
            tracing::info!("accepted connection from {}", addr);

            match noise::handshake_as_responder(&mut stream, &secret).await {
                Ok((transport, remote_pub)) => {
                    tracing::info!("Noise handshake succeeded (as responder)");
                    let Some(peer) = discovery::get_discovered_by_address(&addr.ip().to_string())
                    else {
                        tracing::warn!(
                            "incoming peer {} has no discovered metadata; closing connection",
                            addr
                        );
                        continue;
                    };

                    if let Err(error) = session::run_session(
                        stream,
                        transport,
                        my_pub,
                        remote_pub,
                        &peer.device_id,
                        &peer.device_name,
                        &peer.device_type,
                        confirmer.clone(),
                    )
                    .await
                    {
                        tracing::warn!("session error: {error:#}");
                    }
                }
                Err(error) => tracing::warn!("handshake failed: {error:#}"),
            }
        }
    });

    Ok(())
}

uniffi::include_scaffolding!("hyprconnect");
