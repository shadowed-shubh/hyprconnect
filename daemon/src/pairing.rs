use std::time::{SystemTime, UNIX_EPOCH};
use anyhow:: Result;
use protocol::packet::NetworkPacket;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct PairBody {
    pub pair: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// Generic over "anything writable" so it works whether we're handed
// a whole stream or just half of a split one.
pub async fn request_pairing<W: AsyncWrite + Unpin>(writer: &mut W) -> Result<i64> {
    let ts = now_secs();
    let body = serde_json::to_value(PairBody {
        pair: true,
        timestamp: Some(ts),
    })?;
    let packet = NetworkPacket {
        id: 0,
        packet_type: "kdeconnect.pair".to_string(),
        body,
    };
    writer.write_all(packet.to_line()?.as_bytes()).await?;
    Ok(ts)
}

// Reads lines until it finds a kdeconnect.pair response, skipping
// anything else that might arrive in between.
pub async fn wait_for_pair_response<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<bool> {
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            anyhow::bail!("connection closed while waiting for pair response");
        }
        let Ok(packet) = NetworkPacket::from_line(&line) else {
            continue;
        };
        if packet.packet_type != "kdeconnect.pair" {
            info!("ignoring non-pair packet while waiting: {}", packet.packet_type);
            continue;
        }
        let body: PairBody = serde_json::from_value(packet.body)?;
        return Ok(body.pair);
    }
}

// Informational only for now — sorts the two certs so both sides
// would compute the same value regardless of who's "first".
pub fn verification_code(cert_a: &[u8], cert_b: &[u8], timestamp: i64) -> String {
    let (first, second) = if cert_a <= cert_b {
        (cert_a, cert_b)
    } else {
        (cert_b, cert_a)
    };
    let mut hasher = Sha256::new();
    hasher.update(first);
    hasher.update(second);
    hasher.update(timestamp.to_string().as_bytes());
    hex::encode(&hasher.finalize()[..4])
}

