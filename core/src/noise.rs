use anyhow::{Context, Result};
use snow::{Builder, HandshakeState, TransportState};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

// This exact string picks the cryptographic building blocks: X25519
// for key exchange, ChaChaPoly for encryption, BLAKE2s for hashing.
// Must be identical on both sides or the handshake fails outright.
const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

async fn read_frame<R: AsyncRead + Unpin>(stream: &mut R) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let len = u16::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn write_frame<W: AsyncWrite + Unpin>(stream: &mut W, data: &[u8]) -> Result<()> {
    let len = u16::try_from(data.len()).context("frame too large for u16 length prefix")?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(data).await?;
    Ok(())
}

pub async fn send_encrypted<W: AsyncWrite + Unpin>(
    stream: &mut W,
    transport: &mut TransportState,
    plaintext: &[u8],
) -> Result<()> {
    let mut buf = vec![0u8; plaintext.len() + 16];
    let len = transport.write_message(plaintext, &mut buf)?;
    write_frame(stream, &buf[..len]).await?;
    Ok(())
}

pub async fn recv_encrypted<R: AsyncRead + Unpin>(
    stream: &mut R,
    transport: &mut TransportState,
) -> Result<Vec<u8>> {
    let frame = read_frame(stream).await?;
    let mut buf = vec![0u8; frame.len()];
    let len = transport.read_message(&frame, &mut buf)?;
    buf.truncate(len);
    Ok(buf)
}

pub async fn handshake_as_initiator(
    stream: &mut TcpStream,
    static_secret: &[u8; 32],
) -> Result<(TransportState, [u8; 32])> {
    let mut handshake: HandshakeState = Builder::new(NOISE_PARAMS.parse()?)
        .local_private_key(static_secret)
        .build_initiator()?;

    let mut buf = vec![0u8; 1024];

    let len = handshake.write_message(&[], &mut buf)?;
    write_frame(stream, &buf[..len]).await?;

    let msg2 = read_frame(stream).await?;
    handshake.read_message(&msg2, &mut buf)?;

    let len = handshake.write_message(&[], &mut buf)?;
    write_frame(stream, &buf[..len]).await?;

    // Capture the peer's authenticated static key BEFORE converting
    // to transport mode — this is what pairing fingerprints are
    // built from, no separate identity exchange needed.
    let remote_pub: [u8; 32] = handshake
        .get_remote_static()
        .context("no remote static key after handshake")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("remote static key wrong length"))?;

    Ok((handshake.into_transport_mode()?, remote_pub))
}

// Same change for handshake_as_responder:
pub async fn handshake_as_responder(
    stream: &mut TcpStream,
    static_secret: &[u8; 32],
) -> Result<(TransportState, [u8; 32])> {
    let mut handshake: HandshakeState = Builder::new(NOISE_PARAMS.parse()?)
        .local_private_key(static_secret)
        .build_responder()?;

    let mut buf = vec![0u8; 1024];

    let msg1 = read_frame(stream).await?;
    handshake.read_message(&msg1, &mut buf)?;

    let len = handshake.write_message(&[], &mut buf)?;
    write_frame(stream, &buf[..len]).await?;

    let msg3 = read_frame(stream).await?;
    handshake.read_message(&msg3, &mut buf)?;

    let remote_pub: [u8; 32] = handshake
        .get_remote_static()
        .context("no remote static key after handshake")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("remote static key wrong length"))?;

    Ok((handshake.into_transport_mode()?, remote_pub))
}
