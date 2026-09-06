use anyhow::{Context, Result};
use snow::{Builder, HandshakeState, TransportState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

// This exact string picks the cryptographic building blocks: X25519
// for key exchange, ChaChaPoly for encryption, BLAKE2s for hashing.
// Must be identical on both sides or the handshake fails outright.
const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

// Reads one length-prefixed frame: 2 bytes (how many bytes follow),
// then that many bytes. This replaces "read a line" now that our
// data is encrypted binary, not readable text.
async fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let len = u16::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn write_frame(stream: &mut TcpStream, data: &[u8]) -> Result<()> {
    let len = u16::try_from(data.len()).context("frame too large for u16 length prefix")?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(data).await?;
    Ok(())
}

// Runs the Noise_XX handshake as the INITIATOR — the side that dials
// out. Unlike TLS, there's no role confusion here: whoever calls
// connect() naturally becomes the Noise initiator, no reversal needed.
pub async fn handshake_as_initiator(
    stream: &mut TcpStream,
    static_secret: &[u8; 32],
) -> Result<TransportState> {
    let mut handshake: HandshakeState = Builder::new(NOISE_PARAMS.parse()?)
        .local_private_key(static_secret)
        .build_initiator()?;

    // Noise_XX is a 3-message handshake: -> e, <- e ee s es, -> s se
    let mut buf = vec![0u8; 1024];

    // Message 1: we send our ephemeral key.
    let len = handshake.write_message(&[], &mut buf)?;
    write_frame(stream, &buf[..len]).await?;

    // Message 2: receive their ephemeral + static key.
    let msg2 = read_frame(stream).await?;
    handshake.read_message(&msg2, &mut buf)?;

    // Message 3: send our static key, finishing the handshake.
    let len = handshake.write_message(&[], &mut buf)?;
    write_frame(stream, &buf[..len]).await?;

    Ok(handshake.into_transport_mode()?)
}

// Same handshake, RESPONDER side — whoever accepted the connection.
pub async fn handshake_as_responder(
    stream: &mut TcpStream,
    static_secret: &[u8; 32],
) -> Result<TransportState> {
    let mut handshake: HandshakeState = Builder::new(NOISE_PARAMS.parse()?)
        .local_private_key(static_secret)
        .build_responder()?;

    let mut buf = vec![0u8; 1024];

    // Message 1: receive their ephemeral key.
    let msg1 = read_frame(stream).await?;
    handshake.read_message(&msg1, &mut buf)?;

    // Message 2: send our ephemeral + static key.
    let len = handshake.write_message(&[], &mut buf)?;
    write_frame(stream, &buf[..len]).await?;

    // Message 3: receive their static key, finishing the handshake.
    let msg3 = read_frame(stream).await?;
    handshake.read_message(&msg3, &mut buf)?;

    Ok(handshake.into_transport_mode()?)
}

// Once handshake is done, use these to send/receive real packets —
// encrypts/decrypts, and reuses the same length-prefix framing.
pub async fn send_encrypted(
    stream: &mut TcpStream,
    transport: &mut TransportState,
    plaintext: &[u8],
) -> Result<()> {
    let mut buf = vec![0u8; plaintext.len() + 16]; // +16 for auth tag
    let len = transport.write_message(plaintext, &mut buf)?;
    write_frame(stream, &buf[..len]).await?;
    Ok(())
}

pub async fn recv_encrypted(
    stream: &mut TcpStream,
    transport: &mut TransportState,
) -> Result<Vec<u8>> {
    let frame = read_frame(stream).await?;
    let mut buf = vec![0u8; frame.len()];
    let len = transport.read_message(&frame, &mut buf)?;
    buf.truncate(len);
    Ok(buf)
}
