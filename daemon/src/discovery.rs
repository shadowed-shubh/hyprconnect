use std::collections::HashSet;
use std::time::Duration;

use anyhow::{Context, Result};
use protocol::{identity::Identity, packet::NetworkPacket, KDECONNECT_PORT};
use rustls::pki_types::CertificateDer;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{TcpStream, UdpSocket};
use tokio_rustls::TlsAcceptor;
use tracing::{info, warn};

pub async fn run(
    my_identity: Identity,
    acceptor: TlsAcceptor,
    my_cert_der: CertificateDer<'static>,
) -> Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", KDECONNECT_PORT))
        .await
        .context("failed to bind UDP port 1716")?;
    socket.set_broadcast(true)?;

    info!("listening for devices on port {}", KDECONNECT_PORT);

    let packet = NetworkPacket {
        id: 0,
        packet_type: "kdeconnect.identity".to_string(),
        body: serde_json::to_value(&my_identity)?,
    };
    let line = packet.to_line()?;

    tokio::select! {
        result = announce_loop(&socket, &line) => result,
        result = listen_loop(&socket, my_identity, acceptor, my_cert_der) => result,
    }
}

async fn announce_loop(socket: &UdpSocket, line: &str) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        socket
            .send_to(line.as_bytes(), ("255.255.255.255", KDECONNECT_PORT))
            .await?;
    }
}

async fn listen_loop(
    socket: &UdpSocket,
    my_identity: Identity,
    acceptor: TlsAcceptor,
    my_cert_der: CertificateDer<'static>,
) -> Result<()> {
    let mut buf = vec![0u8; 8192];
    let mut already_connected: HashSet<String> = HashSet::new();

    loop {
        let (n, from) = socket.recv_from(&mut buf).await?;
        let text = String::from_utf8_lossy(&buf[..n]);

        let Ok(packet) = NetworkPacket::from_line(&text) else {
            continue;
        };
        let Ok(identity) = serde_json::from_value::<Identity>(packet.body) else {
            continue;
        };

        if identity.device_id == my_identity.device_id {
            continue;
        }
        if already_connected.contains(&identity.device_id) {
            continue;
        }
        already_connected.insert(identity.device_id.clone());

        info!(
            "discovered device: {} ({}) from {}",
            identity.device_name, identity.device_id, from
        );

        let target_ip = from.ip();
        let target_port = identity.tcp_port;
        let my_identity_clone = my_identity.clone();
        let remote_identity_clone = identity.clone();
        let acceptor_clone = acceptor.clone();
        let my_cert_clone = my_cert_der.clone();

        tokio::spawn(async move {
            if let Err(e) = connect_to_device(
                target_ip,
                target_port,
                my_identity_clone,
                my_cert_clone,
                remote_identity_clone,
                acceptor_clone,
            )
            .await
            {
                warn!("failed to connect to device: {e:#}");
            }
        });
    }
}

async fn connect_to_device(
    ip: std::net::IpAddr,
    port: u16,
    my_identity: Identity,
    my_cert_der: CertificateDer<'static>,
    remote_identity: Identity,
    acceptor: TlsAcceptor,
) -> Result<()> {
    let mut stream = TcpStream::connect((ip, port))
        .await
        .context("TCP connect failed")?;

    info!("connected via TCP to {}:{}", ip, port);

    let mut plaintext_body = serde_json::to_value(&my_identity)?;
    if let serde_json::Value::Object(ref mut map) = plaintext_body {
        map.insert(
            "targetDeviceId".to_string(),
            serde_json::json!(remote_identity.device_id),
        );
        map.insert(
            "targetProtocolVersion".to_string(),
            serde_json::json!(remote_identity.protocol_version),
        );
    }
    let plaintext_packet = NetworkPacket {
        id: 0,
        packet_type: "kdeconnect.identity".to_string(),
        body: plaintext_body,
    };
    stream
        .write_all(plaintext_packet.to_line()?.as_bytes())
        .await?;
    stream.flush().await?;

    info!("sent plaintext identity, starting TLS as server");

    let tls_stream = crate::tls::accept(&acceptor, stream)
        .await
        .context("TLS handshake failed")?;

    info!("TLS handshake succeeded with {}:{}", ip, port);

    let remote_cert_der = tls_stream
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|certs| certs.first())
        .map(|c| c.to_vec())
        .context("no peer certificate presented")?;

    let (read_half, mut write_half) = tokio::io::split(tls_stream);
    let mut reader = BufReader::new(read_half);

    let secure_identity_packet = NetworkPacket {
        id: 0,
        packet_type: "kdeconnect.identity".to_string(),
        body: serde_json::to_value(&my_identity)?,
    };
    write_half
        .write_all(secure_identity_packet.to_line()?.as_bytes())
        .await?;
    info!("sent identity over TLS");

    let mut identity_line = String::new();
    reader.read_line(&mut identity_line).await?;
    info!("received over TLS: {}", identity_line.trim());

    let is_trusted = crate::trust::is_trusted(&remote_identity.device_id, &remote_cert_der);

    if is_trusted {
        info!(
            "{} is already trusted — starting session",
            remote_identity.device_name
        );
    } else {
        info!("requesting pairing with {}", remote_identity.device_name);
        let ts = crate::pairing::request_pairing(&mut write_half).await?;
        let accepted = crate::pairing::wait_for_pair_response(&mut reader).await?;

        if !accepted {
            warn!("{} rejected pairing", remote_identity.device_name);
            return Ok(());
        }

        let code = crate::pairing::verification_code(&my_cert_der, &remote_cert_der, ts);
        info!(
            "PAIRED with {} — verification code: {}",
            remote_identity.device_name, code
        );
        crate::trust::add_trusted(
            &remote_identity.device_id,
            &remote_identity.device_name,
            &remote_cert_der,
        )?;
    }

    // Connection is now trusted (freshly paired or already known) —
    // keep it open and actually do something with it.
    run_session(&mut reader, &mut write_half, &remote_identity.device_name).await
}

// Runs for as long as the connection stays open: sends one ping to
// prove we can send authenticated commands, then keeps listening
// for whatever the phone sends us, logging anything interesting.
async fn run_session<R, W>(reader: &mut R, writer: &mut W, device_name: &str) -> Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let ping = NetworkPacket {
        id: 0,
        packet_type: "kdeconnect.ping".to_string(),
        body: serde_json::json!({}),
    };
    writer.write_all(ping.to_line()?.as_bytes()).await?;
    info!("sent ping to {}", device_name);

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            info!("{} disconnected", device_name);
            return Ok(());
        }

        let Ok(packet) = NetworkPacket::from_line(&line) else {
            continue;
        };

        match packet.packet_type.as_str() {
            "kdeconnect.ping" => info!("received ping from {}", device_name),
            other => info!("received packet from {}: {}", device_name, other),
        }
    }
}
