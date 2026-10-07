use muxy_client::Client;
use muxy_server::{Registry, ServerSettings, connection};
use std::error::Error;
use std::os::unix::{ffi::OsStrExt, net::UnixStream};
use std::sync::{Arc, mpsc};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn uploads_arrive_in_chunks_and_keep_their_name_on_the_server() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (send, events) = mpsc::channel();
    let registry = Arc::new(
        Registry::new(ServerSettings::default(), send)
            .with_uploads(directory.path().join("uploads")),
    );
    let (local, remote) = UnixStream::pair()?;
    let serving = std::thread::spawn(move || connection::serve(Box::new(remote), registry, events));
    let client = Client::from_stream(Box::new(local))?;
    let session =
        client.create_session(directory.path(), muxy_protocol::Size { cols: 20, rows: 4 })?;
    let bytes: Vec<u8> = (0..muxy_protocol::MAX_UPLOAD_CHUNK * 5 / 2)
        .map(|index| u8::try_from(index % 251).unwrap_or_default())
        .collect();
    let path = client.upload(session.id, "dir/shot.png", &bytes)?;
    let path = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&path.0));
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("shot.png")
    );
    assert_eq!(std::fs::read(&path)?, bytes);
    let empty = client.upload(session.id, "empty.txt", &[])?;
    assert_eq!(std::fs::read(std::ffi::OsStr::from_bytes(&empty.0))?, b"");
    let unknown = muxy_protocol::SessionId::new(session.id.get() + 100).ok_or("session")?;
    assert!(client.upload(unknown, "a", b"x").is_err());
    let too_big = vec![0; usize::try_from(muxy_protocol::MAX_UPLOAD_BYTES)? + 1];
    assert!(matches!(
        client.upload(session.id, "big", &too_big),
        Err(muxy_client::ClientError::Invalid(_))
    ));
    client.end_session(session.id)?;
    drop(client);
    serving.join().map_err(|_| "server thread panicked")??;
    Ok(())
}
