//! TLS for a host half's `wss://` (rustls), and the look at a certificate curl refused: an address the user listed is
//! trusted by the key they pinned for it (`plugins::pins`), else by the system's store; a certificate refused is met —
//! its key kept for the user to trust (`pins::saw`) — and named in the error. A pinned key is the only thing checked
//! besides the server proving it holds that key (the handshake's signature): neither the name nor the dates.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned};

use crate::plugins;

/// How long reaching a server and its handshake may take.
pub(super) const CONNECT: Duration = Duration::from_secs(15);

/// A certificate's key as `pins` writes it: `sha256//` and the base64 of its SubjectPublicKeyInfo's SHA-256.
pub(super) fn key_of(cert: &CertificateDer<'_>) -> Option<String> {
    use base64::Engine as _;
    use sha2::Digest as _;
    let ee = webpki::EndEntityCert::try_from(cert).ok()?;
    Some(format!("sha256//{}", base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(ee.subject_public_key_info().as_ref()))))
}

/// Trusts the pinned key alone, or (none pinned) what the system trusts; either way notes the key it met.
#[derive(Debug)]
struct Verifier {
    pin: Option<String>,
    system: Option<Arc<dyn ServerCertVerifier>>,
    provider: Arc<CryptoProvider>,
    met: Mutex<Option<String>>,
}

impl ServerCertVerifier for Verifier {
    fn verify_server_cert(&self, ee: &CertificateDer<'_>, others: &[CertificateDer<'_>], name: &ServerName<'_>, ocsp: &[u8], now: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        let key = key_of(ee);
        *self.met.lock().unwrap_or_else(|e| e.into_inner()) = key.clone();
        match (&self.pin, &self.system) {
            (Some(pin), _) if key.as_deref() == Some(pin.as_str()) => Ok(ServerCertVerified::assertion()),
            (Some(_), _) => Err(rustls::Error::General("not the key you trusted".into())),
            (None, Some(system)) => system.verify_server_cert(ee, others, name, ocsp, now),
            (None, None) => Err(rustls::Error::General("no certificate store".into())),
        }
    }
    fn verify_tls12_signature(&self, m: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(m, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, m: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(m, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// A TCP connection to `host:port`, the first of its addresses that answers.
pub(super) fn tcp(host: &str, port: u16) -> Result<TcpStream> {
    let mut last = anyhow!("{host} has no address");
    for a in (host, port).to_socket_addrs().map_err(|e| anyhow!("{host}: {e}"))? {
        match TcpStream::connect_timeout(&a, CONNECT) {
            Ok(s) => {
                s.set_read_timeout(Some(CONNECT))?;
                s.set_write_timeout(Some(CONNECT))?;
                return Ok(s);
            }
            Err(e) => last = anyhow!("{host}:{port} can't be reached: {e}"),
        }
    }
    Err(last)
}

/// A TLS connection to `host:port` for plugin `id`, its handshake done: with the key the user pinned when the address
/// is one they listed (`listed`), else the system's trust. A certificate refused at a listed address is met
/// (`pins::saw`) and the error says its key and how to trust it.
pub(super) fn connect(id: &str, host: &str, port: u16, listed: bool) -> Result<StreamOwned<ClientConnection, TcpStream>> {
    let hp = format!("{}:{port}", host.to_ascii_lowercase());
    let pin = listed.then(|| plugins::pins::pinned(id, &hp)).flatten();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let system: Option<Arc<dyn ServerCertVerifier>> = match &pin {
        Some(_) => None,
        None => Some(Arc::new(rustls_platform_verifier::Verifier::new(provider.clone()).map_err(|e| anyhow!("the system's certificates can't be read: {e}"))?)),
    };
    let verifier = Arc::new(Verifier { pin: pin.clone(), system, provider: provider.clone(), met: Mutex::new(None) });
    let config = ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions()?.dangerous().with_custom_certificate_verifier(verifier.clone()).with_no_client_auth();
    let name = ServerName::try_from(host.to_string()).map_err(|_| anyhow!("“{host}” isn't a host name"))?;
    let mut conn = ClientConnection::new(Arc::new(config), name)?;
    let mut sock = tcp(host, port)?;
    while conn.is_handshaking() {
        if let Err(e) = conn.complete_io(&mut sock) {
            let met = verifier.met.lock().unwrap_or_else(|e| e.into_inner()).clone();
            return Err(match met {
                Some(key) if listed => explain(id, &hp, pin.as_deref(), &key),
                _ => anyhow!("{hp}: the secure connection failed: {e}"),
            });
        }
    }
    Ok(StreamOwned::new(conn, sock))
}

/// What a certificate refused at a listed address means, its key kept for the user to trust.
fn explain(id: &str, hp: &str, pin: Option<&str>, key: &str) -> anyhow::Error {
    let _ = plugins::pins::saw(id, hp, key);
    let name = plugins::enabled().into_iter().find(|p| p.id == id).map_or_else(|| id.to_string(), |p| p.name);
    let how = format!("trust it in Settings → Plugins → {name}, or with `echoctl plugin trust {id} {hp}`");
    match pin {
        Some(pin) => anyhow!("the certificate at {hp} changed since you trusted it (its key is now {key}; you trusted {pin}): refused until you {how}"),
        None => anyhow!("the certificate at {hp} isn't one this computer trusts (its key: {key}): if it is your own server's, {how}"),
    }
}

/// curl refused `host:port`'s certificate: Echo looks itself — the error naming the key there (met, for the user to
/// trust) when it is refused here too; `None` when it isn't a certificate Echo can name.
pub(super) fn probe(id: &str, hp: &str) -> Option<anyhow::Error> {
    let (host, port) = hp.rsplit_once(':')?;
    let e = connect(id, host, port.parse().ok()?, true).err()?;
    e.to_string().contains("sha256//").then_some(e)
}

/// A `ws://` or `wss://` connection's stream: plain TCP or TLS over it.
pub(super) enum Stream {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Stream {
    pub(super) fn tcp(&self) -> &TcpStream {
        match self {
            Stream::Plain(s) => s,
            Stream::Tls(s) => &s.sock,
        }
    }
}

impl Read for Stream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(b),
            Stream::Tls(s) => s.read(b),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(b),
            Stream::Tls(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}
