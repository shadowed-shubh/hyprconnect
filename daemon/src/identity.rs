use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use x25519_dalek::StaticSecret;

pub struct DeviceIdentity {
    pub device_id: String,
    pub ed25519_signing: SigningKey,
    pub ed25519_verifying: VerifyingKey,
    pub x25519_secret: StaticSecret,
    pub x25519_public: [u8; 32],
}

#[derive(Serialize, Deserialize)]
struct SavedIdentity {
    device_id: String,
    ed25519_secret_hex: String,
    x25519_secret_hex: String,
}

fn identity_file_path() -> Result<PathBuf> {
    let mut dir = dirs::config_dir().context("could not determine config directory")?;
    dir.push("hyprconnect");
    fs::create_dir_all(&dir)?;
    dir.push("identity.json");
    Ok(dir)
}

fn generate(device_id: &str) -> Result<DeviceIdentity> {
    let mut ed_secret_bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut OsRng, &mut ed_secret_bytes);
    let ed_signing = SigningKey::from_bytes(&ed_secret_bytes);
    let ed_verifying = ed_signing.verifying_key();

    let x_secret = StaticSecret::random_from_rng(OsRng);
    let x_public = x25519_dalek::PublicKey::from(&x_secret);

    Ok(DeviceIdentity {
        device_id: device_id.to_string(),
        ed25519_signing: ed_signing,
        ed25519_verifying: ed_verifying,
        x25519_secret: x_secret,
        x25519_public: x_public.to_bytes(),
    })
}

pub fn load_or_generate() -> Result<DeviceIdentity> {
    let path = identity_file_path()?;

    if path.exists() {
        let text = fs::read_to_string(&path)?;
        let saved: SavedIdentity = serde_json::from_str(&text)?;

        let ed_secret_bytes: [u8; 32] = hex::decode(&saved.ed25519_secret_hex)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("ed25519 secret must be 32 bytes"))?;
        let ed_signing = SigningKey::from_bytes(&ed_secret_bytes);
        let ed_verifying = ed_signing.verifying_key();

        let x_secret_bytes: [u8; 32] = hex::decode(&saved.x25519_secret_hex)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("x25519 secret must be 32 bytes"))?;
        let x_secret = StaticSecret::from(x_secret_bytes);
        let x_public = x25519_dalek::PublicKey::from(&x_secret);

        return Ok(DeviceIdentity {
            device_id: saved.device_id,
            ed25519_signing: ed_signing,
            ed25519_verifying: ed_verifying,
            x25519_secret: x_secret,
            x25519_public: x_public.to_bytes(),
        });
    }

    let device_id = uuid::Uuid::new_v4().simple().to_string();
    let identity = generate(&device_id)?;

    let saved = SavedIdentity {
        device_id: identity.device_id.clone(),
        ed25519_secret_hex: hex::encode(identity.ed25519_signing.to_bytes()),
        x25519_secret_hex: hex::encode(identity.x25519_secret.to_bytes()),
    };
    fs::write(&path, serde_json::to_string_pretty(&saved)?)?;

    Ok(identity)
}
