// Keeps track of which devices we've paired with before, so we
// don't ask to pair again every time we reconnect. "Trust" here
// means: we've seen this exact certificate before, tied to this
// device ID, and a human already accepted it once.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDevice {
    pub device_id: String,
    pub device_name: String,
    // Certificates are raw bytes; hex is just a readable way to
    // store bytes in a JSON text file.
    pub cert_der_hex: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TrustStore {
    devices: HashMap<String, TrustedDevice>,
}

fn trust_file_path() -> Result<PathBuf> {
    let mut dir = dirs::config_dir().context("could not determine config directory")?;
    dir.push("hyprconnect");
    fs::create_dir_all(&dir)?;
    dir.push("trusted_devices.json");
    Ok(dir)
}

fn load() -> Result<TrustStore> {
    let path = trust_file_path()?;
    if !path.exists() {
        return Ok(TrustStore::default());
    }
    let text = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

fn save(store: &TrustStore) -> Result<()> {
    let path = trust_file_path()?;
    fs::write(path, serde_json::to_string_pretty(store)?)?;
    Ok(())
}

pub fn is_trusted(device_id: &str, cert_der: &[u8]) -> bool {
    let Ok(store) = load() else { return false };
    match store.devices.get(device_id) {
        Some(d) => d.cert_der_hex == hex::encode(cert_der),
        None => false,
    }
}

pub fn add_trusted(device_id: &str, device_name: &str, cert_der: &[u8]) -> Result<()> {
    let mut store = load()?;
    store.devices.insert(
        device_id.to_string(),
        TrustedDevice {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            cert_der_hex: hex::encode(cert_der),
        },
    );
    save(&store)
}
