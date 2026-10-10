//! Constants that define the HyprConnect wire protocol.
//!
//! These values are interoperability contracts, not user configuration.

pub(crate) const SERVICE_TYPE: &str = "_hyprconnect._tcp.local.";
pub(crate) const TXT_DEVICE_ID: &str = "device_id";
pub(crate) const TXT_DEVICE_NAME: &str = "device_name";
pub(crate) const TXT_DEVICE_TYPE: &str = "device_type";
pub(crate) const TXT_PROTOCOL_VERSION: &str = "protocol_version";

pub(crate) const PROTOCOL_VERSION: u32 = 1;
pub(crate) const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
pub(crate) const FINGERPRINT_MODULUS: u32 = 1_000_000;
pub(crate) const FRAME_LENGTH_BYTES: usize = 2;

pub(crate) fn protocol_version_is_compatible(value: Option<&str>) -> bool {
    value.and_then(|version| version.parse::<u32>().ok()) == Some(PROTOCOL_VERSION)
}

#[cfg(test)]
mod tests {
    use super::{protocol_version_is_compatible, PROTOCOL_VERSION};

    #[test]
    fn accepts_current_protocol_version() {
        assert!(protocol_version_is_compatible(Some("1")));
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    #[test]
    fn rejects_missing_invalid_and_unknown_versions() {
        assert!(!protocol_version_is_compatible(None));
        assert!(!protocol_version_is_compatible(Some("invalid")));
        assert!(!protocol_version_is_compatible(Some("2")));
    }
}
