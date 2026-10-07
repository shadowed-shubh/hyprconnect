use std::sync::Arc;

use anyhow::Result;
use snow::TransportState;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

use crate::features;
use crate::packet::{recv_packet, send_packet, Packet};
use crate::pairing::{self, PairingConfirmer};
use crate::trust;

pub async fn run_session(
    stream: TcpStream,
    transport: TransportState,
    my_pub: [u8; 32],
    remote_pub: [u8; 32],
    remote_id: &str,
    remote_name: &str,
    remote_type: &str,
    confirmer: Arc<dyn PairingConfirmer>,
) -> Result<()> {
    if trust::is_trusted(&remote_pub) {
        info!("{} is already trusted", remote_name);
    } else {
        let code = pairing::fingerprint(&my_pub, &remote_pub);
        if confirmer.confirm(remote_id, &code, remote_name).await? {
            trust::add_trusted(&remote_pub, remote_id, remote_name, remote_type)?;
            info!("paired with {}", remote_name);
        } else {
            warn!("pairing rejected for {}", remote_name);
            return Ok(());
        }
    }

    let (mut read_half, mut write_half) = stream.into_split();
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Packet>(32);
    let transport = Arc::new(Mutex::new(transport));
    let writer_transport = transport.clone();
    let writer_name = remote_name.to_string();

    let writer_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(packet) = outbound_rx.recv() => {
                    let mut t = writer_transport.lock().await;
                    match send_packet(&mut write_half, &mut t, &packet).await {
                        Ok(()) => info!("sent {} to {}", packet.packet_type, writer_name),
                        Err(e) => {
                            warn!("failed to send {} to {}: {e:#}", packet.packet_type, writer_name);
                            break;
                        }
                    }
                }
                else => break,
            }
        }
    });

    outbound_tx.send(features::ping::ping()).await?;

    loop {
        let packet = {
            let mut t = transport.lock().await;
            match recv_packet(&mut read_half, &mut t).await {
                Ok(packet) => packet,
                Err(e) => {
                    info!("session with {} ended: {e:#}", remote_name);
                    break;
                }
            }
        };

        if features::ping::is_ping(&packet) {
            info!("received PING from {}, replying PONG", remote_name);
            outbound_tx.send(features::ping::pong()).await?;
        } else if features::ping::is_pong(&packet) {
            info!("received PONG from {} — round trip confirmed", remote_name);
        } else {
            info!(
                "received unknown packet type '{}' from {}, ignoring",
                packet.packet_type, remote_name
            );
        }
    }

    writer_task.abort();
    Ok(())
}
