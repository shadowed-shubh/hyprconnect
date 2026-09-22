use anyhow::Result;
use serde_json::json;
use snow::TransportState;
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::packet::{recv_packet, send_packet, Packet};
use crate::{pairing, trust};

pub async fn run_session(
    stream: &mut TcpStream,
    mut transport: TransportState,
    my_pub: [u8; 32],
    remote_pub: [u8; 32],
    remote_name: &str,
) -> Result<()> {
    if trust::is_trusted(&remote_pub) {
        info!("{} is already trusted", remote_name);
    } else {
        let code = pairing::fingerprint(&my_pub, &remote_pub);
        if pairing::confirm_via_stdin(&code, remote_name) {
            trust::add_trusted(&remote_pub, remote_name)?;
            info!("paired with {}", remote_name);
        } else {
            warn!("pairing rejected for {}", remote_name);
            return Ok(());
        }
    }

    // Prove the round trip: send a PING, then dispatch whatever
    // comes back based on packet type. This loop is the pattern
    // every future feature (battery, clipboard, notifications)
    // will reuse — read a packet, match on type, route it.
    send_packet(stream, &mut transport, &Packet::new("PING", json!({}))).await?;
    info!("sent PING to {}", remote_name);

    loop {
        let packet = match recv_packet(stream, &mut transport).await {
            Ok(p) => p,
            Err(e) => {
                info!("session with {} ended: {e:#}", remote_name);
                return Ok(());
            }
        };

        match packet.packet_type.as_str() {
            "PING" => {
                info!("received PING from {}, replying PONG", remote_name);
                send_packet(stream, &mut transport, &Packet::new("PONG", json!({}))).await?;
            }
            "PONG" => {
                info!("received PONG from {} — round trip confirmed", remote_name);
            }
            other => {
                // Unknown packet types are logged and ignored, not
                // fatal — this IS the versioning mechanism per
                // SPEC.md §14, not a bug.
                info!(
                    "received unknown packet type '{}' from {}, ignoring",
                    other, remote_name
                );
            }
        }
    }
}
