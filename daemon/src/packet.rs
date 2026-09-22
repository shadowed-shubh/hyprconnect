use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snow::TransportState;
use tokio::net::TcpStream;

use crate::noise;

// Matches SPEC.md's wire format. Since Noise already frames each
// encrypted message as one discrete unit (length-prefixed), we don't
// need newline-delimiting on top — one Packet = one encrypted message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    #[serde(rename = "type")]
    pub packet_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_id: Option<u32>,
    pub body: Value,
}

impl Packet {
    pub fn new(packet_type: impl Into<String>, body: Value) -> Self {
        Self {
            packet_type: packet_type.into(),
            req_id: None,
            body,
        }
    }
}

pub async fn send_packet(
    stream: &mut TcpStream,
    transport: &mut TransportState,
    packet: &Packet,
) -> Result<()> {
    let bytes = serde_json::to_vec(packet)?;
    noise::send_encrypted(stream, transport, &bytes).await
}

pub async fn recv_packet(
    stream: &mut TcpStream,
    transport: &mut TransportState,
) -> Result<Packet> {
    let bytes = noise::recv_encrypted(stream, transport).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
