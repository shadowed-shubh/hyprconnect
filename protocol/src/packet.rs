use serde::{Deserialize, Serialize};

// This is the outer wrapper for every message on the wire — both
// identity broadcasts and, later, everything else (battery, ping,
// clipboard...). "body" holds whatever data belongs to that
// specific packet type, and we don't know its shape ahead of time,
// so it's a raw JSON value here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPacket {
    pub id: i64,

    #[serde(rename = "type")]
    pub packet_type: String,

    pub body: serde_json::Value,
}

impl NetworkPacket {
    // Turns this packet into one line of text ready to send over
    // the wire. The wire format is one JSON object per line.
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }

    // Reverse of to_line — parses one line of text back into a packet.
    pub fn from_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line.trim_end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let packet = NetworkPacket {
            id: 12345,
            packet_type: "kdeconnect.ping".to_string(),
            body: serde_json::json!({}),
        };

        let line = packet.to_line().unwrap();
        assert!(line.ends_with('\n'));

        let back = NetworkPacket::from_line(&line).unwrap();
        assert_eq!(back.packet_type, "kdeconnect.ping");
        assert_eq!(back.id, 12345);
    }
}
