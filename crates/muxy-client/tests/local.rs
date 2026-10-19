use muxy_client::{Client, ClientError, local};
use std::io::{self, Read};
use std::time::{Duration, Instant};

#[test]
fn handshake_timeout_cancels_the_reader_before_another_startup_attempt() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let socket = directory.path().join("server.sock");
    let listener = muxy_protocol::transport::LocalListener::bind(&socket)?;
    let server = std::thread::spawn(move || -> io::Result<bool> {
        let (mut stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut bytes = [0; 256];
        assert!(stream.read(&mut bytes)? > 0);
        Ok(stream.read(&mut bytes)? == 0)
    });
    let start = Instant::now();
    assert!(matches!(
        Client::connect_with_timeout(&socket, Duration::from_millis(50)),
        Err(ClientError::Timeout)
    ));
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(
        server
            .join()
            .map_err(|_| io::Error::other("server thread failed"))??
    );
    assert!(local::unavailable(&ClientError::Io(
        io::ErrorKind::ConnectionRefused.into()
    )));
    assert!(!local::unavailable(&ClientError::VersionUnsupported));
    assert!(!local::unavailable(&ClientError::Io(
        io::ErrorKind::PermissionDenied.into()
    )));
    Ok(())
}

#[test]
fn a_server_elsewhere_is_explained_and_not_retried() {
    use muxy_client::ServerElsewhere;
    for place in [
        ServerElsewhere::Session,
        ServerElsewhere::Elevated,
        ServerElsewhere::NotElevated,
    ] {
        let error = ClientError::ServerElsewhere(place);
        assert!(!error.recoverable());
        assert!(!local::unavailable(&error));
        assert!(error.to_string().contains("muxy server stop"), "{error}");
    }
}
