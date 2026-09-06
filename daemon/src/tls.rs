use std::sync::Arc;

use anyhow::Result;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::server::WebPkiClientVerifier;
use rustls::{DistinguishedName, RootCertStore, ServerConfig, SignatureScheme};
use tokio::net::TcpStream;
use tokio_rustls::{server::TlsStream, TlsAcceptor};

// Same idea as before — accept any client certificate, since real
// trust comes from the human-confirmed pairing code later, not
// certificate validation now. This time it's the SERVER side of
// verification, since we're acting as the TLS server.
#[derive(Debug)]
struct AcceptAnyClientCert;

impl ClientCertVerifier for AcceptAnyClientCert {
    fn offer_client_auth(&self) -> bool {
        true
    }
    fn client_auth_mandatory(&self) -> bool {
        false
    }
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }
    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ED25519,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::RSA_PKCS1_SHA256,
        ]
    }
}

// Now builds a SERVER config instead of a client config — we're
// acting as the TLS server on this connection, per the protocol's
// reversed-role rule.
pub fn make_acceptor(
    cert_der: CertificateDer<'static>,
    key_der: PrivatePkcs8KeyDer<'static>,
) -> Result<TlsAcceptor> {
    let verifier = WebPkiClientVerifier::builder(Arc::new(RootCertStore::empty()))
        .allow_unauthenticated()
        .build()
        .unwrap_or_else(|_| Arc::new(AcceptAnyClientCertFallback));

    let _ = verifier; // placeholder in case builder path differs by version

    let config = ServerConfig::builder()
        .with_client_cert_verifier(Arc::new(AcceptAnyClientCert))
        .with_single_cert(vec![cert_der], key_der.into())?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

// Fallback type only used if WebPkiClientVerifier path doesn't apply
// in your rustls version — safe to ignore/remove if unused warning appears.
#[derive(Debug)]
struct AcceptAnyClientCertFallback;
impl ClientCertVerifier for AcceptAnyClientCertFallback {
    fn offer_client_auth(&self) -> bool { true }
    fn client_auth_mandatory(&self) -> bool { false }
    fn root_hint_subjects(&self) -> &[DistinguishedName] { &[] }
    fn verify_client_cert(&self, _e: &CertificateDer<'_>, _i: &[CertificateDer<'_>], _n: rustls::pki_types::UnixTime) -> Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, _m: &[u8], _c: &CertificateDer<'_>, _d: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(&self, _m: &[u8], _c: &CertificateDer<'_>, _d: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { vec![] }
}

// Upgrade the TCP connection to TLS, acting as SERVER this time.
pub async fn accept(acceptor: &TlsAcceptor, stream: TcpStream) -> Result<TlsStream<TcpStream>> {
    let tls_stream = acceptor.accept(stream).await?;
    Ok(tls_stream)
}
