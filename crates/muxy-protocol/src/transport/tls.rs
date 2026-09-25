//! TLS 1.3 over TCP for paired devices, pinned to the server's certificate.
//!
//! A rustls connection needs exclusive access for both reading and writing, so
//! one pump thread moves bytes between TLS and one end of a Unix socket pair.
//! The other end is the byte stream callers split, exactly like a local socket.

use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, SignatureScheme};

use crate::transport::{ByteStream, Listener, StreamCancellation};

const ALPN: &[u8] = b"muxy/1";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
const BUFFER: usize = 64 * 1024;
const KEEPALIVE_IDLE: Duration = Duration::from_secs(30);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);
const KEEPALIVE_PROBES: u32 = 3;

/// A server certificate and its PKCS#8 private key, both DER encoded.
#[derive(Clone, Eq, PartialEq)]
pub struct TlsIdentity {
    pub certificate: Vec<u8>,
    pub private_key: Vec<u8>,
}

impl fmt::Debug for TlsIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TlsIdentity")
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}

impl TlsIdentity {
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(&self.certificate)
    }
}

/// SHA-256 of a DER certificate, the value phones pin.
pub fn fingerprint(certificate: &[u8]) -> [u8; 32] {
    let digest = ring::digest::digest(&ring::digest::SHA256, certificate);
    let mut bytes = [0; 32];
    bytes.copy_from_slice(digest.as_ref());
    bytes
}

/// The server presented a certificate other than the pinned one.
#[derive(Debug)]
pub struct IdentityMismatch;

impl fmt::Display for IdentityMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the server's identity no longer matches this pairing")
    }
}

impl Error for IdentityMismatch {}

pub fn is_identity_mismatch(error: &io::Error) -> bool {
    matches!(error.get_ref(), Some(inner) if inner.is::<IdentityMismatch>())
}

#[derive(Debug)]
pub struct TlsListener {
    socket: Mutex<Option<TcpListener>>,
    closed: Condvar,
    config: Arc<rustls::ServerConfig>,
    address: SocketAddr,
}

impl TlsListener {
    pub fn bind(address: SocketAddr, identity: &TlsIdentity) -> io::Result<Self> {
        let config = server_config(identity)?;
        let socket = TcpListener::bind(address)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            address: socket.local_addr()?,
            socket: Mutex::new(Some(socket)),
            closed: Condvar::new(),
            config,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }
}

impl Listener for TlsListener {
    fn accept(&self) -> io::Result<Box<dyn ByteStream>> {
        let mut guard = self
            .socket
            .lock()
            .map_err(|_| io::Error::other("listener lock poisoned"))?;
        loop {
            let socket = guard
                .as_ref()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "listener is closed"))?;
            match socket.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false)?;
                    return Ok(Box::new(PendingTls::new(stream, Arc::clone(&self.config))?));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    (guard, _) = self
                        .closed
                        .wait_timeout(guard, Duration::from_millis(10))
                        .map_err(|_| io::Error::other("listener lock poisoned"))?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn close(&self) {
        let mut guard = self.socket.lock().unwrap_or_else(PoisonError::into_inner);
        guard.take();
        self.closed.notify_all();
    }
}

/// Connects to a paired server, trusting only the pinned certificate.
pub fn connect(
    host: &str,
    port: u16,
    fingerprint: [u8; 32],
    timeout: Duration,
) -> io::Result<Box<dyn ByteStream>> {
    let mut failure = io::Error::new(io::ErrorKind::NotFound, "host has no addresses");
    let mut connected = None;
    for address in (host, port).to_socket_addrs()? {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(stream) => {
                connected = Some(stream);
                break;
            }
            Err(error) => failure = error,
        }
    }
    let mut tcp = connected.ok_or(failure)?;
    let name = ServerName::try_from("muxy.invalid").map_err(io::Error::other)?;
    let connection = rustls::ClientConnection::new(client_config(fingerprint)?, name)
        .map_err(io::Error::other)?;
    let connection = handshake(rustls::Connection::Client(connection), &mut tcp, timeout)
        .map_err(identity_error)?;
    let (app, pump) = socket_pair()?;
    start(connection, tcp, pump, &AtomicBool::new(false))?;
    Ok(Box::new(app))
}

struct PendingTls {
    tcp: TcpStream,
    config: Arc<rustls::ServerConfig>,
    app: UnixStream,
    pump: UnixStream,
    pumping: Arc<AtomicBool>,
}

impl PendingTls {
    fn new(tcp: TcpStream, config: Arc<rustls::ServerConfig>) -> io::Result<Self> {
        let (app, pump) = socket_pair()?;
        Ok(Self {
            tcp,
            config,
            app,
            pump,
            pumping: Arc::default(),
        })
    }
}

impl ByteStream for PendingTls {
    fn cancellation(&self) -> io::Result<Box<dyn StreamCancellation>> {
        Ok(Box::new(PendingCancellation {
            app: self.app.try_clone()?,
            tcp: self.tcp.try_clone()?,
            pumping: Arc::clone(&self.pumping),
        }))
    }

    #[allow(clippy::type_complexity)]
    fn split(self: Box<Self>) -> io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
        let Self {
            mut tcp,
            config,
            app,
            pump,
            pumping,
        } = *self;
        let connection = rustls::ServerConnection::new(config).map_err(io::Error::other)?;
        let connection = handshake(
            rustls::Connection::Server(connection),
            &mut tcp,
            HANDSHAKE_TIMEOUT,
        )?;
        start(connection, tcp, pump, &pumping)?;
        Box::new(app).split()
    }
}

/// Before the pump starts, cancelling also cuts TCP so a stalled handshake
/// ends at once. Afterwards only the app end closes, so the pump still flushes
/// what the connection wrote last.
struct PendingCancellation {
    app: UnixStream,
    tcp: TcpStream,
    pumping: Arc<AtomicBool>,
}

impl StreamCancellation for PendingCancellation {
    fn cancel(&self) {
        let _ = self.app.shutdown(Shutdown::Both);
        if !self.pumping.load(Ordering::Acquire) {
            let _ = self.tcp.shutdown(Shutdown::Both);
        }
    }
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn server_config(identity: &TlsIdentity) -> io::Result<Arc<rustls::ServerConfig>> {
    let certificate = CertificateDer::from(identity.certificate.clone());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(identity.private_key.clone()));
    let mut config = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(io::Error::other)?
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    config.alpn_protocols = vec![ALPN.to_vec()];
    config.send_tls13_tickets = 0;
    config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
    Ok(Arc::new(config))
}

fn client_config(fingerprint: [u8; 32]) -> io::Result<Arc<rustls::ClientConfig>> {
    let provider = provider();
    let verifier = Arc::new(PinnedCertificate {
        fingerprint,
        provider: Arc::clone(&provider),
    });
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(io::Error::other)?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.alpn_protocols = vec![ALPN.to_vec()];
    config.enable_sni = false;
    config.resumption = rustls::client::Resumption::disabled();
    Ok(Arc::new(config))
}

/// Trusts exactly one certificate, while still checking that the peer holds its key.
#[derive(Debug)]
struct PinnedCertificate {
    fingerprint: [u8; 32],
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if fingerprint(end_entity) == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General("TLS 1.2 is not supported".into()))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn identity_error(error: io::Error) -> io::Error {
    let mismatch = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>())
        .is_some_and(|inner| {
            matches!(
                inner,
                rustls::Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)
            )
        });
    if mismatch {
        io::Error::new(io::ErrorKind::InvalidData, IdentityMismatch)
    } else {
        error
    }
}

/// Every read and write gets only the time left, so a peer that trickles bytes
/// cannot stretch the handshake past its deadline.
fn handshake(
    mut connection: rustls::Connection,
    tcp: &mut TcpStream,
    timeout: Duration,
) -> io::Result<rustls::Connection> {
    let deadline = Instant::now() + timeout;
    loop {
        while connection.wants_write() {
            tcp.set_write_timeout(Some(remaining(deadline)?))?;
            connection.write_tls(tcp)?;
        }
        if !connection.is_handshaking() {
            break;
        }
        tcp.set_read_timeout(Some(remaining(deadline)?))?;
        if connection.read_tls(tcp)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        connection
            .process_new_packets()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    }
    if connection.alpn_protocol() != Some(ALPN) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "peer did not negotiate the muxy protocol",
        ));
    }
    tcp.set_read_timeout(None)?;
    tcp.set_write_timeout(None)?;
    Ok(connection)
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| io::ErrorKind::TimedOut.into())
}

fn socket_pair() -> io::Result<(UnixStream, UnixStream)> {
    let (app, pump) = UnixStream::pair()?;
    // std sets no SO_NOSIGPIPE on Apple socket pairs, and an iOS host app does
    // not ignore SIGPIPE the way Rust executables do.
    #[cfg(target_vendor = "apple")]
    for socket in [&app, &pump] {
        rustix::net::sockopt::set_socket_nosigpipe(socket, true)?;
    }
    Ok((app, pump))
}

fn start(
    mut connection: rustls::Connection,
    tcp: TcpStream,
    pump: UnixStream,
    pumping: &AtomicBool,
) -> io::Result<()> {
    tcp.set_nodelay(true)?;
    rustix::net::sockopt::set_socket_keepalive(&tcp, true)?;
    rustix::net::sockopt::set_tcp_keepidle(&tcp, KEEPALIVE_IDLE)?;
    rustix::net::sockopt::set_tcp_keepintvl(&tcp, KEEPALIVE_INTERVAL)?;
    rustix::net::sockopt::set_tcp_keepcnt(&tcp, KEEPALIVE_PROBES)?;
    tcp.set_nonblocking(true)?;
    pump.set_nonblocking(true)?;
    // Plaintext is only accepted while TLS has nothing queued, which bounds it.
    connection.set_buffer_limit(None);
    pumping.store(true, Ordering::Release);
    let pump = Pump {
        tls: connection,
        tcp,
        app: pump,
        received: Vec::new(),
        buffer: vec![0; BUFFER],
        inbound: Flow::Open,
        outbound: Flow::Open,
        deadline: None,
    };
    thread::Builder::new()
        .name("tls-pump".into())
        .spawn(move || pump.run())?;
    Ok(())
}

/// One direction of the pumped connection.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Flow {
    Open,
    /// The source finished; buffered bytes are still being forwarded.
    Draining,
    /// The destination's write side has been shut down.
    Done,
}

struct Pump {
    tls: rustls::Connection,
    tcp: TcpStream,
    app: UnixStream,
    /// Decrypted bytes not yet accepted by the app end.
    received: Vec<u8>,
    buffer: Vec<u8>,
    /// Network to app.
    inbound: Flow,
    /// App to network.
    outbound: Flow,
    deadline: Option<Instant>,
}

impl Pump {
    fn run(mut self) {
        let _ = self.pump();
        let _ = self.tcp.shutdown(Shutdown::Both);
        let _ = self.app.shutdown(Shutdown::Both);
    }

    fn pump(&mut self) -> io::Result<()> {
        loop {
            self.receive()?;
            self.deliver();
            self.send()?;
            self.transmit()?;
            if self.finished() {
                return Ok(());
            }
            self.wait()?;
        }
    }

    fn receive(&mut self) -> io::Result<()> {
        self.decrypted()?;
        while self.inbound == Flow::Open && self.received.len() < BUFFER {
            match self.tls.read_tls(&mut self.tcp) {
                Ok(0) => self.inbound = Flow::Draining,
                Ok(_) => {
                    self.tls
                        .process_new_packets()
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    self.decrypted()?;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn decrypted(&mut self) -> io::Result<()> {
        loop {
            match self.tls.reader().read(&mut self.buffer) {
                Ok(0) => break,
                Ok(read) => self.received.extend_from_slice(&self.buffer[..read]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(error) => return Err(error),
            }
        }
        if self.inbound == Flow::Open {
            self.inbound = Flow::Draining;
        }
        Ok(())
    }

    fn deliver(&mut self) {
        while !self.received.is_empty() {
            match self.app.write(&self.received) {
                Ok(written) => {
                    self.received.drain(..written);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => {
                    // The connection stopped reading: nothing more can be delivered.
                    self.received.clear();
                    self.inbound = Flow::Done;
                    self.close_outbound();
                    return;
                }
            }
        }
        if self.inbound == Flow::Draining {
            let _ = self.app.shutdown(Shutdown::Write);
            self.inbound = Flow::Done;
        }
    }

    fn send(&mut self) -> io::Result<()> {
        while self.outbound == Flow::Open && !self.tls.wants_write() {
            match self.app.read(&mut self.buffer) {
                Ok(0) => self.close_outbound(),
                Ok(read) => self.tls.writer().write_all(&self.buffer[..read])?,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => self.close_outbound(),
            }
        }
        Ok(())
    }

    fn close_outbound(&mut self) {
        if self.outbound == Flow::Open {
            self.outbound = Flow::Draining;
            self.tls.send_close_notify();
            self.deadline = Some(Instant::now() + CLOSE_TIMEOUT);
        }
    }

    fn transmit(&mut self) -> io::Result<()> {
        while self.tls.wants_write() {
            match self.tls.write_tls(&mut self.tcp) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        if self.outbound == Flow::Draining {
            let _ = self.tcp.shutdown(Shutdown::Write);
            self.outbound = Flow::Done;
        }
        Ok(())
    }

    fn finished(&self) -> bool {
        (self.inbound == Flow::Done && self.outbound == Flow::Done)
            || self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn wait(&self) -> io::Result<()> {
        let mut network = PollFlags::empty();
        if self.inbound == Flow::Open && self.received.len() < BUFFER {
            network |= PollFlags::IN;
        }
        if self.tls.wants_write() {
            network |= PollFlags::OUT;
        }
        let mut local = PollFlags::empty();
        if self.outbound == Flow::Open && !self.tls.wants_write() {
            local |= PollFlags::IN;
        }
        if !self.received.is_empty() {
            local |= PollFlags::OUT;
        }
        // Only watch descriptors with pending interest; a hung-up socket would
        // otherwise wake poll immediately, forever.
        let mut descriptors = Vec::with_capacity(2);
        if !network.is_empty() {
            descriptors.push(PollFd::new(&self.tcp, network));
        }
        if !local.is_empty() {
            descriptors.push(PollFd::new(&self.app, local));
        }
        if descriptors.is_empty() {
            thread::sleep(Duration::from_millis(10));
            return Ok(());
        }
        let timeout = self
            .deadline
            .map(|deadline| Timespec::try_from(deadline.saturating_duration_since(Instant::now())))
            .transpose()
            .map_err(io::Error::other)?;
        match rustix::event::poll(&mut descriptors, timeout.as_ref()) {
            Ok(_) | Err(rustix::io::Errno::INTR) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}
