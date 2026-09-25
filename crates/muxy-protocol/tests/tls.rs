use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use muxy_protocol::transport::tls::{self, TlsIdentity, TlsListener, is_identity_mismatch};
use muxy_protocol::transport::{ByteStream, Listener, StreamCancellation};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
type Halves = (Box<dyn Read + Send>, Box<dyn Write + Send>);

const TIMEOUT: Duration = Duration::from_secs(5);

fn identity() -> Result<TlsIdentity> {
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    let certificate = rcgen::CertificateParams::default().self_signed(&key)?;
    Ok(TlsIdentity {
        certificate: certificate.der().to_vec(),
        private_key: key.serialize_der(),
    })
}

fn listen(identity: &TlsIdentity) -> Result<Arc<TlsListener>> {
    Ok(Arc::new(TlsListener::bind(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        identity,
    )?))
}

fn accept_one(listener: &Arc<TlsListener>) -> mpsc::Receiver<io::Result<Halves>> {
    let (sender, receiver) = mpsc::channel();
    let listener = Arc::clone(listener);
    thread::spawn(move || {
        let _ = sender.send(listener.accept().and_then(ByteStream::split));
    });
    receiver
}

fn connect(listener: &TlsListener, identity: &TlsIdentity) -> io::Result<Box<dyn ByteStream>> {
    tls::connect(
        "127.0.0.1",
        listener.local_addr().port(),
        identity.fingerprint(),
        TIMEOUT,
    )
}

fn pair() -> Result<(Halves, Halves)> {
    let identity = identity()?;
    let listener = listen(&identity)?;
    let server = accept_one(&listener);
    let client = connect(&listener, &identity)?.split()?;
    Ok((client, server.recv_timeout(TIMEOUT)??))
}

#[test]
fn bytes_round_trip_through_split_halves() -> Result {
    let ((mut client_read, mut client_write), (mut server_read, mut server_write)) = pair()?;
    client_write.write_all(b"hello")?;
    let mut received = [0; 5];
    server_read.read_exact(&mut received)?;
    assert_eq!(&received, b"hello");
    server_write.write_all(b"world")?;
    client_read.read_exact(&mut received)?;
    assert_eq!(&received, b"world");
    Ok(())
}

#[test]
fn closing_one_writer_ends_that_direction_only() -> Result {
    let ((mut client_read, client_write), (mut server_read, mut server_write)) = pair()?;
    drop(client_write);
    let mut received = Vec::new();
    server_read.read_to_end(&mut received)?;
    assert!(received.is_empty());
    server_write.write_all(b"bye")?;
    drop(server_write);
    client_read.read_to_end(&mut received)?;
    assert_eq!(received, b"bye");
    Ok(())
}

/// Blocks a reader and a writer on one end, then checks that cancelling that end wakes both.
fn cancelling_wakes(cancellation: &dyn StreamCancellation, halves: Halves) -> Result {
    let (mut read, mut write) = halves;
    let (done, finished) = mpsc::channel();
    let reading = done.clone();
    let reader = thread::spawn(move || {
        let _ = read.read(&mut [0; 16]);
        let _ = reading.send(());
    });
    let writer = thread::spawn(move || {
        let chunk = vec![0; 1024 * 1024];
        while write.write_all(&chunk).is_ok() {}
        let _ = done.send(());
    });
    thread::sleep(Duration::from_millis(100));
    cancellation.cancel();
    finished.recv_timeout(TIMEOUT)?;
    finished.recv_timeout(TIMEOUT)?;
    reader.join().map_err(|_| "reader panicked")?;
    writer.join().map_err(|_| "writer panicked")?;
    Ok(())
}

#[test]
fn cancellation_wakes_blocked_readers_and_writers_on_both_ends() -> Result {
    let identity = identity()?;
    let listener = listen(&identity)?;
    for cancel_server in [false, true] {
        let accepted = {
            let listener = Arc::clone(&listener);
            thread::spawn(move || -> io::Result<_> {
                let stream = listener.accept()?;
                let cancellation = stream.cancellation()?;
                Ok((cancellation, stream.split()?))
            })
        };
        let client = connect(&listener, &identity)?;
        let client_cancellation = client.cancellation()?;
        let client_halves = client.split()?;
        let (server_cancellation, server_halves) =
            accepted.join().map_err(|_| "accept thread panicked")??;
        if cancel_server {
            let _peer = client_halves;
            cancelling_wakes(server_cancellation.as_ref(), server_halves)?;
        } else {
            let _peer = server_halves;
            cancelling_wakes(client_cancellation.as_ref(), client_halves)?;
        }
    }
    Ok(())
}

#[test]
fn a_trickling_server_cannot_stretch_the_client_handshake() -> Result {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let server = thread::spawn(move || -> io::Result<()> {
        let (mut stream, _) = listener.accept()?;
        // A handshake record header promising 16 KiB, then one byte at a time.
        stream.write_all(&[0x16, 0x03, 0x03, 0x40, 0x00])?;
        for _ in 0..100 {
            thread::sleep(Duration::from_millis(50));
            if stream.write_all(&[0]).is_err() {
                break;
            }
        }
        Ok(())
    });
    let started = Instant::now();
    let result = tls::connect("127.0.0.1", port, [0; 32], Duration::from_millis(500));
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(3));
    let _ = server.join();
    Ok(())
}

#[test]
fn closing_the_listener_wakes_accept() -> Result {
    let listener = listen(&identity()?)?;
    let accepted = accept_one(&listener);
    thread::sleep(Duration::from_millis(50));
    listener.close();
    let error = accepted
        .recv_timeout(TIMEOUT)?
        .err()
        .ok_or("accept succeeded")?;
    assert_eq!(error.kind(), io::ErrorKind::NotConnected);
    Ok(())
}

#[test]
fn a_certificate_other_than_the_pinned_one_is_an_identity_mismatch() -> Result {
    let listener = listen(&identity()?)?;
    let _server = accept_one(&listener);
    let other = identity()?;
    let error = tls::connect(
        "127.0.0.1",
        listener.local_addr().port(),
        other.fingerprint(),
        TIMEOUT,
    )
    .err()
    .ok_or("connected with the wrong pin")?;
    assert!(is_identity_mismatch(&error), "{error}");
    Ok(())
}

#[test]
fn peers_that_do_not_negotiate_the_muxy_protocol_are_rejected() -> Result {
    let identity = identity()?;
    let listener = listen(&identity)?;
    for protocols in [Vec::new(), vec![b"http/1.1".to_vec()]] {
        let server = accept_one(&listener);
        let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyCertificate))
        .with_no_client_auth();
        config.alpn_protocols = protocols;
        let mut tcp = TcpStream::connect(listener.local_addr())?;
        tcp.set_read_timeout(Some(TIMEOUT))?;
        let mut connection =
            rustls::ClientConnection::new(Arc::new(config), "muxy.invalid".try_into()?)?;
        while connection.is_handshaking() {
            if connection.complete_io(&mut tcp).is_err() {
                break;
            }
        }
        assert!(server.recv_timeout(TIMEOUT)?.is_err());
    }
    Ok(())
}

#[test]
fn cancelling_a_stalled_handshake_ends_it_promptly() -> Result {
    let listener = listen(&identity()?)?;
    let accepted = {
        let listener = Arc::clone(&listener);
        thread::spawn(move || -> io::Result<_> {
            let stream = listener.accept()?;
            let cancellation = stream.cancellation()?;
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let _ = sender.send(stream.split().map(|_| ()));
            });
            Ok((cancellation, receiver))
        })
    };
    let _silent = TcpStream::connect(listener.local_addr())?;
    let (cancellation, handshake) = accepted.join().map_err(|_| "accept thread panicked")??;
    thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    cancellation.cancel();
    assert!(handshake.recv_timeout(TIMEOUT)?.is_err());
    assert!(started.elapsed() < TIMEOUT);
    Ok(())
}

#[test]
fn large_transfers_in_both_directions_arrive_intact() -> Result {
    const SERVER_BYTES: usize = 16 * 1024 * 1024;
    const CLIENT_BYTES: usize = 4 * 1024 * 1024;
    let pattern = |length: usize| -> Vec<u8> {
        (0..length)
            .map(|index| u8::try_from(index % 251).unwrap_or_default())
            .collect()
    };
    let ((mut client_read, mut client_write), (mut server_read, mut server_write)) = pair()?;
    let upload = thread::spawn(move || client_write.write_all(&pattern(CLIENT_BYTES)));
    let download = thread::spawn(move || server_write.write_all(&pattern(SERVER_BYTES)));
    let receive = thread::spawn(move || -> io::Result<Vec<u8>> {
        let mut received = vec![0; CLIENT_BYTES];
        server_read.read_exact(&mut received)?;
        Ok(received)
    });
    let mut received = vec![0; SERVER_BYTES];
    for chunk in received.chunks_mut(1024 * 1024) {
        client_read.read_exact(chunk)?;
        thread::sleep(Duration::from_millis(5));
    }
    assert!(received == pattern(SERVER_BYTES));
    assert!(receive.join().map_err(|_| "receiver panicked")?? == pattern(CLIENT_BYTES));
    upload.join().map_err(|_| "upload panicked")??;
    download.join().map_err(|_| "download panicked")??;
    Ok(())
}

#[derive(Debug)]
struct AnyCertificate;

impl rustls::client::danger::ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
