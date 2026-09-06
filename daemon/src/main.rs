mod cert;
mod discovery;
mod pairing;
mod tls;
mod trust;

use protocol::identity::Identity;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let (device_id, device_cert) = cert::load_or_generate()?;
    let my_cert_der = device_cert.cert_der.clone();
    let acceptor = tls::make_acceptor(device_cert.cert_der, device_cert.key_der)?;

    let my_identity = Identity {
        device_id: device_id.clone(),
        device_name: "Shubh's Desktop".to_string(),
        device_type: "desktop".to_string(),
        protocol_version: 8,
        tcp_port: 1716,
        incoming_capabilities: vec![],
        outgoing_capabilities: vec![],
    };

    discovery::run(my_identity, acceptor, my_cert_der).await
}
