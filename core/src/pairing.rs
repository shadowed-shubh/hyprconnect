use std::sync::Mutex;

use futures_util::future::BoxFuture;
use once_cell::sync::OnceCell;
use sha2::{Digest, Sha256};
use tokio::sync::oneshot;
use tracing::warn;

use crate::protocol::FINGERPRINT_MODULUS;
use crate::{DeviceEventCallback, HyprConnectError};

const PAIRING_CONFIRMATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

fn clear_pending_pairing_if_matches(device_id: &str) {
    let mut pending = pending_slot().lock().unwrap();
    if pending
        .as_ref()
        .is_some_and(|pending| pending.device_id == device_id)
    {
        pending.take();
    }
}

pub trait PairingConfirmer: Send + Sync {
    fn confirm(
        &self,
        device_id: &str,
        code: &str,
        remote_name: &str,
    ) -> BoxFuture<'static, Result<bool, HyprConnectError>>;
}

pub struct StdinConfirmer;

impl PairingConfirmer for StdinConfirmer {
    fn confirm(
        &self,
        _device_id: &str,
        code: &str,
        remote_name: &str,
    ) -> BoxFuture<'static, Result<bool, HyprConnectError>> {
        let code = code.to_string();
        let remote_name = remote_name.to_string();
        Box::pin(async move { Ok(confirm_via_stdin(&code, &remote_name)) })
    }
}

pub struct CallbackConfirmer;

/// Accepts every pairing request without asking the user.
///
/// Selected by `confirmation = "auto"` in `config.toml`. Never the default:
/// auto-accepting defeats the TOFU check that the numeric code provides.
pub struct AcceptAllConfirmer;

impl PairingConfirmer for AcceptAllConfirmer {
    fn confirm(
        &self,
        device_id: &str,
        _code: &str,
        remote_name: &str,
    ) -> BoxFuture<'static, Result<bool, HyprConnectError>> {
        let device_id = device_id.to_string();
        let remote_name = remote_name.to_string();
        Box::pin(async move {
            tracing::info!("auto-accepting pairing with {remote_name} ({device_id})");
            Ok(true)
        })
    }
}

struct PendingPairing {
    device_id: String,
    responder: oneshot::Sender<bool>,
}

static CALLBACK: OnceCell<Mutex<Option<Box<dyn DeviceEventCallback>>>> = OnceCell::new();
static PENDING: OnceCell<Mutex<Option<PendingPairing>>> = OnceCell::new();

fn callback_slot() -> &'static Mutex<Option<Box<dyn DeviceEventCallback>>> {
    CALLBACK.get_or_init(|| Mutex::new(None))
}

fn pending_slot() -> &'static Mutex<Option<PendingPairing>> {
    PENDING.get_or_init(|| Mutex::new(None))
}

pub fn is_pairing_in_progress() -> bool {
    pending_slot().lock().unwrap().is_some()
}

pub fn register_callback(callback: Box<dyn DeviceEventCallback>) {
    *callback_slot().lock().unwrap() = Some(callback);
}

pub fn confirm_pairing(device_id: String, accepted: bool) {
    let pending = pending_slot().lock().unwrap().take();
    match pending {
        Some(pending) if pending.device_id == device_id => {
            let _ = pending.responder.send(accepted);
        }
        Some(pending) => {
            warn!(
                "ignoring pairing confirmation for stale device {}",
                device_id
            );
            *pending_slot().lock().unwrap() = Some(pending);
        }
        None => {
            warn!("ignoring pairing confirmation with no pending pairing");
        }
    }
}

impl PairingConfirmer for CallbackConfirmer {
    fn confirm(
        &self,
        device_id: &str,
        code: &str,
        _remote_name: &str,
    ) -> BoxFuture<'static, Result<bool, HyprConnectError>> {
        let device_id = device_id.to_string();
        let code = code.to_string();

        Box::pin(async move {
            let (sender, receiver) = oneshot::channel();
            {
                let mut pending = pending_slot().lock().unwrap();
                if pending.is_some() {
                    return Err(HyprConnectError::PairingInProgress);
                }
                *pending = Some(PendingPairing {
                    device_id: device_id.clone(),
                    responder: sender,
                });
            }

            {
                let callback = callback_slot().lock().unwrap();
                let Some(callback) = callback.as_ref() else {
                    pending_slot().lock().unwrap().take();
                    return Err(HyprConnectError::Transport(
                        "pairing callback is not registered".to_string(),
                    ));
                };
                callback.on_pairing_code(device_id.clone(), code);
            }

            match tokio::time::timeout(PAIRING_CONFIRMATION_TIMEOUT, receiver).await {
                Ok(result) => result.map_err(|_| {
                    HyprConnectError::Transport("pairing confirmation channel closed".to_string())
                }),
                Err(_) => {
                    warn!("pairing confirmation timed out for device {device_id}");
                    clear_pending_pairing_if_matches(&device_id);
                    Err(HyprConnectError::Transport(
                        "pairing confirmation timed out after 60 seconds".to_string(),
                    ))
                }
            }
        })
    }
}

pub fn fingerprint(my_pub: &[u8; 32], remote_pub: &[u8; 32]) -> String {
    let (first, second) = if my_pub <= remote_pub {
        (my_pub, remote_pub)
    } else {
        (remote_pub, my_pub)
    };
    let mut hasher = Sha256::new();
    hasher.update(first);
    hasher.update(second);
    let hash = hasher.finalize();
    let code = u32::from_be_bytes([0, hash[0], hash[1], hash[2]]);
    format!("{:06}", code % FINGERPRINT_MODULUS)
}

pub fn confirm_via_stdin(code: &str, remote_name: &str) -> bool {
    println!(
        "Pairing request from {} — verification code: {}",
        remote_name, code
    );
    println!("Does this match on both devices? [y/N]: ");

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    input.trim().eq_ignore_ascii_case("y")
}

#[cfg(test)]
mod tests {
    use super::{clear_pending_pairing_if_matches, pending_slot, PendingPairing};
    use tokio::sync::oneshot;

    #[test]
    fn pending_cleanup_does_not_remove_a_different_pairing() {
        let (sender, _receiver) = oneshot::channel();
        *pending_slot().lock().unwrap() = Some(PendingPairing {
            device_id: "new-device".to_string(),
            responder: sender,
        });

        clear_pending_pairing_if_matches("old-device");
        assert!(pending_slot().lock().unwrap().is_some());
        pending_slot().lock().unwrap().take();
    }
}
