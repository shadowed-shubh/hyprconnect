use serde::{Deserialize, Serialize};

// This struct represents one device's identity — the same info
// your phone broadcasts over UDP every few seconds, and the same
// info your desktop broadcasts back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    #[serde(rename = "deviceId")]
    pub device_id: String,

    #[serde(rename = "deviceName")]
    pub device_name: String,

    #[serde(rename = "deviceType")]
    pub device_type: String,

    #[serde(rename = "protocolVersion")]
    pub protocol_version: i32,

    // We never fully confirmed this field ourselves, but the KDE
    // Connect source guarantees it's sent. default_port() means:
    // if it's ever missing, assume 1716 instead of crashing.
    #[serde(rename = "tcpPort", default = "default_port")]
    pub tcp_port: u16,

    #[serde(rename = "incomingCapabilities", default)]
    pub incoming_capabilities: Vec<String>,

    #[serde(rename = "outgoingCapabilities", default)]
    pub outgoing_capabilities: Vec<String>,
}

fn default_port() -> u16 {
    1716
}

// Everything below this line only runs when you type `cargo test`.
// It never runs in the actual program.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_phone_identity() {
        // This is a real body we captured from your phone's broadcast.
        let json = r#"{
            "deviceId": "7b2b392fb6f049199028d5eedfbca84f",
            "deviceName": "OnePlus Nord CE 2 Lite 5G",
            "protocolVersion": 8,
            "deviceType": "phone",
            "incomingCapabilities": ["kdeconnect.battery"],
            "outgoingCapabilities": ["kdeconnect.battery"]
        }"#;

        let identity: Identity = serde_json::from_str(json).unwrap();

        assert_eq!(identity.device_id, "7b2b392fb6f049199028d5eedfbca84f");
        assert_eq!(identity.device_name, "OnePlus Nord CE 2 Lite 5G");
        assert_eq!(identity.protocol_version, 8);
        // tcpPort wasn't in this JSON, so it should fall back to 1716.
        assert_eq!(identity.tcp_port, 1716);
    }
}
