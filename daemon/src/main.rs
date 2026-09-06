mod identity;

use protocol::identity::Identity;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let device_identity = identity::load_or_generate()?;

    let _my_identity = Identity {
        device_id: device_identity.device_id.clone(),
        device_name: "Shubh's Desktop".to_string(),
        device_type: "desktop".to_string(),
        protocol_version: 1,
        tcp_port: 0,
        incoming_capabilities: vec![],
        outgoing_capabilities: vec![],
    };

    println!(
        "device_id: {}",
        device_identity.device_id
    );
    println!(
        "ed25519_pub: {}",
        hex::encode(device_identity.ed25519_verifying.as_bytes())
    );
    println!(
        "x25519_pub: {}",
        hex::encode(device_identity.x25519_public)
    );

    Ok(())
}
