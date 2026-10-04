use super::*;
use std::os::unix::net::UnixStream;

/// A fake server's name, and the input it received or `closed`.
type Heard = (&'static str, String);

#[test]
fn local_and_ssh_workers_keep_their_own_connections_and_generations() -> TestResult {
    let directory = tempfile::Builder::new()
        .prefix("muxy-servers-")
        .tempdir_in("/tmp")?;
    let local_socket = directory.path().join("local.sock");
    let remote_socket = directory.path().join("remote.sock");
    let (heard, hearing) = mpsc::channel();
    for (name, socket) in [("local", &local_socket), ("remote", &remote_socket)] {
        let listener = UnixListener::bind(socket)?;
        let heard = heard.clone();
        thread::spawn(move || listen(&listener, name, &heard));
    }
    let (updates, received) = async_channel::unbounded();
    let remote = ServerId::new();
    let local_work = worker(
        ServerId::local(),
        Target::Local(local_socket),
        updates.clone(),
    )?;
    let remote_work = worker(
        remote,
        Target::Ssh(fake_ssh(directory.path(), &remote_socket)?),
        updates,
    )?;
    local_work.send((1, Work::Connect))?;
    remote_work.send((1, Work::Connect))?;
    connected(&received, &[(ServerId::local(), 1), (remote, 1)])?;

    local_work.send((1, Work::Input(ChannelId(1), b"local".to_vec())))?;
    remote_work.send((1, Work::Input(ChannelId(1), b"remote".to_vec())))?;
    expect(&hearing, &[("local", "local"), ("remote", "remote")])?;

    remote_work.send((2, Work::Connect))?;
    remote_work.send((1, Work::Input(ChannelId(1), b"stale".to_vec())))?;
    connected(&received, &[(remote, 2)])?;
    remote_work.send((2, Work::Input(ChannelId(1), b"fresh".to_vec())))?;
    local_work.send((1, Work::Input(ChannelId(1), b"still".to_vec())))?;
    expect(
        &hearing,
        &[
            ("remote", "closed"),
            ("remote", "fresh"),
            ("local", "still"),
        ],
    )?;

    local_work.send((1, Work::Stop))?;
    remote_work.send((2, Work::Stop))?;
    let rest: Vec<_> =
        std::iter::from_fn(|| hearing.recv_timeout(Duration::from_millis(300)).ok()).collect();
    assert!(
        rest.iter().all(|(_, text)| text == "closed"),
        "stale work reached a server: {rest:?}"
    );
    Ok(())
}

fn listen(listener: &UnixListener, name: &'static str, heard: &Sender<Heard>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            return;
        };
        let heard = heard.clone();
        thread::spawn(move || {
            let _ = record_input(stream, name, &heard);
            let _ = heard.send((name, "closed".into()));
        });
    }
}

fn record_input(stream: UnixStream, name: &'static str, heard: &Sender<Heard>) -> TestResult {
    let mut decoder = Decoder::new(stream.try_clone()?);
    let mut encoder = Encoder::new(stream);
    assert!(matches!(decoder.next()?, (CONTROL, Message::Hello { .. })));
    encoder.send(
        CONTROL,
        &Message::HelloReply {
            versions: SUPPORTED.to_vec(),
            server: muxy_protocol::ServerInfo::current(),
            features: Vec::new(),
        },
    )?;
    identify_desktop(&mut decoder, &mut encoder)?;
    loop {
        match decoder.next()? {
            (
                CONTROL,
                Message::Request {
                    id,
                    body: RequestBody::ListSessions,
                },
            ) => encoder.send(
                CONTROL,
                &Message::Reply {
                    id,
                    body: ReplyBody::Sessions(Vec::new()),
                },
            )?,
            (ChannelId(1), Message::Input(bytes)) => {
                heard.send((name, String::from_utf8(bytes)?))?;
            }
            _ => {}
        }
    }
}

/// Waits until each server reports a connection for its generation.
fn connected(updates: &Updates, expected: &[(ServerId, u64)]) -> TestResult {
    let mut missing = expected.to_vec();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !missing.is_empty() && Instant::now() < deadline {
        match updates.try_recv() {
            Ok((server, generation, Update::Connected(_))) => {
                missing.retain(|pair| *pair != (server, generation));
            }
            Ok((_, _, Update::ConnectFailed(error))) => return Err(error.into()),
            Ok(_) => {}
            Err(_) => thread::sleep(Duration::from_millis(5)),
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("not connected: {missing:?}").into())
    }
}

/// Waits for exactly these reports, in any order.
fn expect(hearing: &mpsc::Receiver<Heard>, expected: &[(&str, &str)]) -> TestResult {
    let mut missing: Vec<_> = expected.to_vec();
    while !missing.is_empty() {
        let (name, text) = hearing.recv_timeout(Duration::from_secs(5))?;
        let position = missing
            .iter()
            .position(|(server, said)| *server == name && *said == text)
            .ok_or_else(|| format!("unexpected report from {name}: {text}"))?;
        missing.remove(position);
    }
    Ok(())
}
