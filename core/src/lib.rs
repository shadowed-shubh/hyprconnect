#![allow(clippy::empty_line_after_doc_comments)]

pub mod discovery;
pub mod features;
pub mod identity;
pub mod noise;
pub mod packet;
pub mod pairing;
pub(crate) mod protocol;
pub mod session;
pub mod settings;
pub mod trust;

#[cfg(test)]
mod test_support;

use once_cell::sync::OnceCell;
use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpListener;
use tracing::info;

static FFI_RUNTIME: OnceCell<tokio::runtime::Runtime> = OnceCell::new();

fn ffi_runtime() -> anyhow::Result<&'static tokio::runtime::Runtime> {
    FFI_RUNTIME.get_or_try_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(Into::into)
    })
}

#[derive(Debug, Error)]
pub enum HyprConnectError {
    #[error("device not found")]
    DeviceNotFound,
    #[error("another pairing is already in progress")]
    PairingInProgress,
    #[error("invalid settings: {0}")]
    InvalidSettings(String),
    #[error("transport error: {0}")]
    Transport(String),
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub trusted: bool,
    pub online: bool,
}

pub trait DeviceEventCallback: Send + Sync {
    fn on_pairing_code(&self, device_id: String, code: String);
}

pub fn set_config_dir(path: String) -> Result<(), HyprConnectError> {
    settings::configure_config_dir(path)
        .map_err(|error| HyprConnectError::InvalidSettings(error.to_string()))
}

pub async fn pair(device_id: String) -> Result<(), HyprConnectError> {
    let runtime =
        ffi_runtime().map_err(|error| HyprConnectError::Transport(format!("{error:#}")))?;
    runtime
        .block_on(pair_impl(device_id))
        .map_err(|error| HyprConnectError::Transport(error.to_string()))
}

async fn pair_impl(device_id: String) -> anyhow::Result<()> {
    if pairing::is_pairing_in_progress() {
        anyhow::bail!("another pairing is already in progress");
    }

    // mDNS can publish the service before the host A record arrives. Give
    // the resolver a short window rather than failing on the first tap.
    let device =
        discovery::get_discovered(&device_id).ok_or_else(|| anyhow::anyhow!("device not found"))?;
    let device = {
        let mut current = device;
        for _ in 0..10 {
            if current.address.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            if let Some(updated) = discovery::get_discovered(&device_id) {
                current = updated;
            }
        }
        current
    };
    let address = device
        .address
        .ok_or_else(|| anyhow::anyhow!("device has no resolved IPv4 address"))?;
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
    .await?;
    Ok(())
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
                online: false,
            },
        );
    }

    for device in discovery::discovered_devices() {
        if let Some(existing) = devices.get_mut(&device.device_id) {
            existing.online = true;
            existing.device_name = device.device_name;
            existing.device_type = device.device_type;
        } else {
            devices.insert(
                device.device_id.clone(),
                DeviceInfo {
                    device_id: device.device_id,
                    device_name: device.device_name,
                    device_type: device.device_type,
                    trusted: false,
                    online: true,
                },
            );
        }
    }

    devices.into_values().collect()
}

pub fn remove_trusted_device(device_id: String) -> Result<(), HyprConnectError> {
    trust::remove_trusted(&device_id)
        .map(|_| ())
        .map_err(|error| HyprConnectError::Transport(error.to_string()))
}

pub fn register_callback(callback: Box<dyn DeviceEventCallback>) {
    pairing::register_callback(callback);
}

pub fn confirm_pairing(device_id: String, accepted: bool) {
    pairing::confirm_pairing(device_id, accepted);
}

/// UniFFI entry point: start mDNS discovery with explicitly supplied settings.
///
/// The config file is not read here; the caller owns the values.
pub async fn start_discovery(
    device_name: String,
    device_type: String,
    auto_pair: bool,
) -> Result<(), HyprConnectError> {
    let settings = settings::Settings::from_args(&device_name, &device_type, auto_pair)
        .map_err(|error| HyprConnectError::InvalidSettings(error.to_string()))?;
    let confirmer: Arc<dyn pairing::PairingConfirmer> = Arc::new(pairing::CallbackConfirmer);
    // UniFFI polls this future from the foreign-language side, which does
    // not provide a Tokio reactor. Keep a runtime alive for mDNS/TCP work.
    ffi_runtime()
        .map_err(|error| HyprConnectError::Transport(format!("{error:#}")))?
        .block_on(start_discovery_with_settings(settings, confirmer))
        .map_err(|error| HyprConnectError::Transport(format!("{error:#}")))
}

/// Start mDNS discovery and the incoming connection listener from settings.
///
/// Used by the daemon after loading `config.toml`.
pub async fn start_discovery_with_settings(
    settings: settings::Settings,
    confirmer: Arc<dyn pairing::PairingConfirmer>,
) -> anyhow::Result<()> {
    settings.validate()?;
    let device_identity = identity::load_or_generate()?;
    let listener = TcpListener::bind((settings.listen_host.as_str(), settings.listen_port)).await?;
    let real_port = listener.local_addr()?.port();
    let secret = device_identity.x25519_secret.to_bytes();
    let my_pub = device_identity.x25519_public;

    info!(
        device_id = %device_identity.device_id,
        device_name = %settings.device_name,
        listen_host = %settings.listen_host,
        listen_port = real_port,
        "daemon listening"
    );

    let discovery_identity = discovery::DiscoveryIdentity {
        device_id: device_identity.device_id,
        device_name: settings.device_name.clone(),
        device_type: settings.device_type.clone(),
        protocol_version: protocol::PROTOCOL_VERSION,
        port: real_port,
        auto_pair: settings.auto_pair,
    };

    tokio::spawn(discovery::run(
        discovery_identity,
        secret,
        my_pub,
        confirmer.clone(),
    ));

    tokio::spawn(async move {
        loop {
            let Ok((stream, addr)) = listener.accept().await else {
                continue;
            };
            tracing::info!("accepted connection from {}", addr);
            let connection_confirmer = confirmer.clone();

            tokio::spawn(async move {
                let mut stream = stream;
                match tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    noise::handshake_as_responder(&mut stream, &secret),
                )
                .await
                {
                    Ok(Ok((transport, remote_pub))) => {
                        tracing::info!("Noise handshake succeeded (as responder)");
                        // Noise authenticates the peer. mDNS only supplies the display and
                        // routing metadata needed by the session; wait briefly for that
                        // metadata instead of treating its absence as authentication failure.
                        let Some(peer) =
                            discovery::wait_for_discovered_by_address(&addr.ip().to_string()).await
                        else {
                            tracing::warn!(
                                "incoming peer {} has no discovered metadata; closing connection",
                                addr
                            );
                            return;
                        };

                        if let Err(error) = session::run_session(
                            stream,
                            transport,
                            my_pub,
                            remote_pub,
                            &peer.device_id,
                            &peer.device_name,
                            &peer.device_type,
                            connection_confirmer,
                        )
                        .await
                        {
                            tracing::warn!("session error: {error:#}");
                        }
                    }
                    Ok(Err(error)) => tracing::warn!("handshake failed: {error:#}"),
                    Err(_) => tracing::warn!("handshake timed out after 10 seconds"),
                }
            });
        }
    });

    Ok(())
}

uniffi::include_scaffolding!("hyprconnect");
