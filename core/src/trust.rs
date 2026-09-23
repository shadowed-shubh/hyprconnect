use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDevice {
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub x25519_pub_hex: String,
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
    Ok(serde_json::from_str(&text)?)
}

fn save(store: &TrustStore) -> Result<()> {
    let path = trust_file_path()?;
    fs::write(path, serde_json::to_string_pretty(store)?)?;
    Ok(())
}

pub fn is_trusted(remote_pub: &[u8; 32]) -> bool {
    let Ok(store) = load() else { return false };
    store.devices.contains_key(&hex::encode(remote_pub))
}

pub fn add_trusted(
    remote_pub: &[u8; 32],
    device_id: &str,
    device_name: &str,
    device_type: &str,
) -> Result<()> {
    let mut store = load()?;
    store.devices.insert(
        hex::encode(remote_pub),
        TrustedDevice {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            device_type: device_type.to_string(),
            x25519_pub_hex: hex::encode(remote_pub),
        },
    );
    save(&store)
}

pub fn trusted_devices() -> Result<Vec<TrustedDevice>> {
    Ok(load()?.devices.into_values().collect())
}
