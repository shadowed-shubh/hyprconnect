use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};

pub struct DeviceCert {
    pub cert_der: CertificateDer<'static>,
    pub key_der: PrivatePkcs8KeyDer<'static>,
}

// What actually gets saved to disk — just the raw bytes, hex-encoded
// so it's readable JSON text instead of binary.
#[derive(Serialize, Deserialize)]
struct SavedIdentity {
    device_id: String,
    cert_der_hex: String,
    key_der_hex: String,
}

fn identity_file_path() -> Result<PathBuf> {
    let mut dir = dirs::config_dir().context("could not determine config directory")?;
    dir.push("hyprconnect");
    fs::create_dir_all(&dir)?;
    dir.push("identity.json");
    Ok(dir)
}

fn generate(device_id: &str) -> Result<DeviceCert> {
    let key_pair = KeyPair::generate()?;

    let mut params = CertificateParams::new(vec![])?;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, device_id);
    params.distinguished_name = dn;

    let cert = params.self_signed(&key_pair)?;

    let cert_der = cert.der().clone();
    let key_der = PrivatePkcs8KeyDer::from(key_pair.serialize_der());

    Ok(DeviceCert { cert_der, key_der })
}

// The real entry point now: load our saved identity if we have one,
// otherwise generate a fresh one and save it so next run reuses it.
// This is what makes us recognizable to devices we've paired with
// before, across restarts.
pub fn load_or_generate() -> Result<(String, DeviceCert)> {
    let path = identity_file_path()?;

    if path.exists() {
        let text = fs::read_to_string(&path)?;
        let saved: SavedIdentity = serde_json::from_str(&text)?;

        let cert_der = CertificateDer::from(hex::decode(&saved.cert_der_hex)?);
        let key_der = PrivatePkcs8KeyDer::from(hex::decode(&saved.key_der_hex)?);

        return Ok((saved.device_id, DeviceCert { cert_der, key_der }));
    }

    let device_id = uuid::Uuid::new_v4().simple().to_string();
    let device_cert = generate(&device_id)?;

    let saved = SavedIdentity {
        device_id: device_id.clone(),
        cert_der_hex: hex::encode(&device_cert.cert_der),
        key_der_hex: hex::encode(device_cert.key_der.secret_pkcs8_der()),
    };
    fs::write(&path, serde_json::to_string_pretty(&saved)?)?;

    Ok((device_id, device_cert))
}
