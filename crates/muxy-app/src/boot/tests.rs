use super::*;
use std::error::Error;
use std::fs;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use muxy_protocol::wire::{Decoder, Encoder};
use muxy_protocol::{CONTROL, Message, ReplyBody, RequestBody, SUPPORTED};

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

#[test]
fn blocked_client_request_does_not_block_input_or_acks_and_flush_waits() -> TestResult {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = PathBuf::from(format!(
        "/tmp/muxy-bridge-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory)?;
    let socket = directory.join("server.sock");
    let listener = UnixListener::bind(&socket)?;
    let (progress, received) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let server = thread::spawn(move || fake_server(&listener, &progress, &gate));
    let (work, updates) = bridge(socket)?;
    work.send((1, Work::Connect))?;
    assert!(matches!(updates.recv_blocking()?.1, Update::ServerInfo(_)));
    assert!(matches!(updates.recv_blocking()?.1, Update::Connected(_)));
    let session = SessionId::from(std::num::NonZeroU64::MIN);
    work.send((
        1,
        Work::ReadSaved {
            pane: PaneId::new(),
            session,
        },
    ))?;
    received.recv_timeout(Duration::from_secs(2))?;
    work.send((1, Work::Input(ChannelId(1), b"input".to_vec())))?;
    work.send((1, Work::Ack(ChannelId(1), 17)))?;
    work.send((1, Work::Flush))?;
    let fast_path_completed = received.recv_timeout(Duration::from_secs(2));
    let early_update = updates.try_recv();
    release.send(())?;
    fast_path_completed?;
    assert!(
        early_update.is_err(),
        "flush must not overtake the blocked request"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saved = false;
    let mut flushed = false;
    while !flushed && Instant::now() < deadline {
        match updates.try_recv() {
            Ok((1, Update::Saved { .. })) => saved = true,
            Ok((1, Update::Flushed)) => {
                assert!(saved);
                flushed = true;
            }
            Ok(other) => return Err(format!("unexpected update: {other:?}").into()),
            Err(_) => thread::sleep(Duration::from_millis(5)),
        }
    }
    work.send((1, Work::Stop))?;
    server.join().map_err(|_| "fake server panicked")??;
    fs::remove_dir_all(directory)?;
    assert!(flushed, "flush must finish after pending work completes");
    Ok(())
}

fn fake_server(
    listener: &UnixListener,
    progress: &Sender<()>,
    gate: &mpsc::Receiver<()>,
) -> TestResult {
    let (socket, _) = listener.accept()?;
    socket.set_read_timeout(Some(Duration::from_secs(3)))?;
    let mut decoder = Decoder::new(socket.try_clone()?);
    let mut encoder = Encoder::new(socket);
    assert!(matches!(decoder.next()?, (CONTROL, Message::Hello { .. })));
    encoder.send(
        CONTROL,
        &Message::HelloReply {
            versions: SUPPORTED.to_vec(),
            server: muxy_protocol::ServerInfo::current(),
        },
    )?;
    identify_desktop(&mut decoder, &mut encoder)?;
    let (
        CONTROL,
        Message::Request {
            id,
            body: RequestBody::ListSessions,
        },
    ) = decoder.next()?
    else {
        return Err("expected listing".into());
    };
    encoder.send(
        CONTROL,
        &Message::Reply {
            id,
            body: ReplyBody::Sessions(Vec::new()),
        },
    )?;
    let (
        CONTROL,
        Message::Request {
            id,
            body: RequestBody::ReadSavedScreen(_),
        },
    ) = decoder.next()?
    else {
        return Err("expected saved request".into());
    };
    progress.send(())?;
    assert_eq!(
        decoder.next()?,
        (ChannelId(1), Message::Input(b"input".to_vec()))
    );
    assert_eq!(
        decoder.next()?,
        (
            CONTROL,
            Message::FrameAck {
                channel: ChannelId(1),
                seq: 17
            }
        )
    );
    progress.send(())?;
    gate.recv_timeout(Duration::from_secs(3))?;
    encoder.send(
        CONTROL,
        &Message::Reply {
            id,
            body: ReplyBody::Error(muxy_protocol::ErrorReply {
                code: ErrorCode::SavedContentUnavailable,
                message: "test record".into(),
            }),
        },
    )?;
    let _ = decoder.next();
    Ok(())
}

#[test]
fn attachment_completion_precedes_early_frames_and_disconnect_without_blocking_other_channels()
-> TestResult {
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 1;
    let first = attachment_update(ChannelId(1))?;
    assert!(matches!(
        delivery.complete(Some(first)).as_slice(),
        [Update::Attached { .. }]
    ));
    delivery.pending = 2;
    let early = frame_event(ChannelId(2));
    assert!(delivery.event(early.clone())?.is_empty());
    assert!(delivery.event(ClientEvent::Disconnected)?.is_empty());
    assert!(matches!(
        delivery.event(frame_event(ChannelId(1)))?.as_slice(),
        [Update::Event(ClientEvent::Frame {
            channel: ChannelId(1),
            ..
        })]
    ));
    assert!(delivery.flush().is_empty());
    let ready = delivery.complete(Some(attachment_update(ChannelId(2))?));
    assert!(
        matches!(ready.as_slice(), [Update::Attached { .. }, Update::Event(event)] if *event == early)
    );
    assert!(matches!(
        delivery.complete(None).as_slice(),
        [Update::Event(ClientEvent::Disconnected), Update::Flushed]
    ));
    Ok(())
}

#[test]
fn deferred_lifecycle_events_are_bounded() {
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 1;
    let mut accepted = 0;
    while delivery
        .event(ClientEvent::SessionEnded {
            session: SessionId::from(std::num::NonZeroU64::MIN),
            reason: muxy_protocol::ExitReason::Ended,
        })
        .is_ok()
    {
        accepted += 1;
        assert!(accepted <= 1024);
    }
    assert_eq!(accepted, 1024);
    assert_eq!(delivery.complete(None).len(), accepted);
}

#[test]
fn exit_closes_known_panes_immediately_and_follows_its_pending_attachment() -> TestResult {
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 2;
    let session = SessionId::from(std::num::NonZeroU64::MIN);
    let event = ClientEvent::SessionEnded {
        session,
        reason: muxy_protocol::ExitReason::Ended,
    };
    assert!(
        matches!(delivery.event(event.clone())?.as_slice(), [Update::CloseSessionPanes(id)] if *id == session)
    );
    let updates = delivery.complete(Some(attachment_update(ChannelId(1))?));
    assert!(
        matches!(updates.as_slice(), [Update::Attached { .. }, Update::Event(ended)] if *ended == event)
    );
    assert_eq!(delivery.pending, 1);
    assert!(delivery.complete(None).is_empty());
    Ok(())
}

fn attachment_update(channel: ChannelId) -> Result<Update, Box<dyn Error + Send + Sync>> {
    let snapshot = Message::samples()
        .into_iter()
        .find_map(|message| match message {
            Message::Reply {
                body: ReplyBody::Attached { snapshot, .. },
                ..
            } => Some(snapshot),
            _ => None,
        })
        .ok_or("missing attachment sample")?;
    Ok(Update::Attached {
        pane: PaneId::new(),
        session: SessionId::from(std::num::NonZeroU64::MIN),
        attachment: Attachment {
            channel,
            grid: muxy_client::RunGrid::from_snapshot(&snapshot),
            title: snapshot.title,
            directory: snapshot.directory,
            process: None,
        },
        created: true,
    })
}

fn frame_event(channel: ChannelId) -> ClientEvent {
    ClientEvent::Frame {
        channel,
        frame: muxy_protocol::ScreenFrame {
            size: Size { cols: 80, rows: 24 },
            graphics: None,
            seq: 1,
            reset: false,
            rows: Vec::new(),
            cursor: muxy_protocol::Cursor {
                shape: muxy_protocol::CursorShape::default(),
                row: 0,
                col: 0,
                visible: true,
            },
            modes: muxy_protocol::Modes::default(),
        },
    }
}

#[test]
fn a_full_event_buffer_preserves_the_only_disconnect_after_history_completion() -> TestResult {
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 1;
    for id in 1..=1024 {
        assert!(matches!(
            delivery
                .event(ClientEvent::SessionEnded {
                    session: SessionId::from(std::num::NonZeroU64::new(id).ok_or("zero session")?),
                    reason: muxy_protocol::ExitReason::Ended,
                })?
                .as_slice(),
            [Update::CloseSessionPanes(_)]
        ));
    }
    assert!(delivery.event(ClientEvent::Disconnected)?.is_empty());
    assert!(delivery.flush().is_empty());
    let ready = delivery.complete(Some(Update::History {
        pane: PaneId::new(),
        request: HistoryRequest::recent(),
        result: Err(ClientError::Disconnected),
    }));
    assert!(matches!(ready.first(), Some(Update::History { .. })));
    assert!(matches!(
        &ready[ready.len() - 2..],
        [Update::Event(ClientEvent::Disconnected), Update::Flushed]
    ));
    assert_eq!(ready.len(), 1027);
    assert!(delivery.event(ClientEvent::Disconnected)?.is_empty());
    Ok(())
}

mod resize;

fn identify_desktop<R: std::io::Read, W: std::io::Write>(
    decoder: &mut Decoder<R>,
    encoder: &mut Encoder<W>,
) -> TestResult {
    let (
        CONTROL,
        Message::Request {
            id,
            body: RequestBody::IdentifyClient(kind),
        },
    ) = decoder.next()?
    else {
        return Err("expected client identity".into());
    };
    assert_eq!(kind, muxy_protocol::ClientKind::Desktop);
    encoder.send(
        CONTROL,
        &Message::Reply {
            id,
            body: ReplyBody::ClientIdentified(muxy_protocol::SessionClient {
                kind,
                ..Default::default()
            }),
        },
    )?;
    Ok(())
}

#[test]
fn deferred_progress_keeps_only_the_latest_state_and_completion_count() -> TestResult {
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 1;
    let session = SessionId::from(std::num::NonZeroU64::MIN);
    let event = |completed| ClientEvent::Progress {
        session,
        progress: muxy_protocol::SessionProgress {
            progress: None,
            completed,
        },
    };
    for completed in 1..=2000 {
        assert!(delivery.event(event(completed))?.is_empty());
    }
    assert!(
        matches!(delivery.complete(None).as_slice(), [Update::Event(received)] if *received == event(2000))
    );
    Ok(())
}

#[test]
fn pending_extension_replies_do_not_block_app_requests_or_flush() -> TestResult {
    let directory = tempfile::Builder::new()
        .prefix("muxy-async-")
        .tempdir_in("/tmp")?;
    let listener = UnixListener::bind(directory.path().join("server.sock"))?;
    let (started, pending) = mpsc::channel();
    let server = thread::spawn(move || withhold_extension_replies(&listener, &started));
    let (work, updates) = bridge(directory.path().join("server.sock"))?;
    work.send((1, Work::Connect))?;
    assert!(matches!(updates.recv_blocking()?.1, Update::ServerInfo(_)));
    assert!(matches!(updates.recv_blocking()?.1, Update::Connected(_)));
    let (reply, client) = async_channel::bounded(1);
    work.send((1, Work::ExtensionClient(reply)))?;
    let client = client.recv_blocking()?.ok_or("missing client")?;
    let project = muxy_protocol::ProjectId::new();
    let files = client.files_async(muxy_protocol::FilesRequest {
        project,
        action: muxy_protocol::FilesAction::List(muxy_protocol::ServerPath(Vec::new())),
    });
    let git = client.git_async(muxy_protocol::GitRequest {
        project,
        action: muxy_protocol::GitAction::Status { local: true },
    });
    let command = client.exec_async(muxy_protocol::ExecRequest {
        job: 1,
        project,
        argv: vec!["pwd".into()],
        shell: None,
        cwd: None,
        stdin: Vec::new(),
        env: std::collections::BTreeMap::new(),
        timeout_ms: 30_000,
    });
    for _ in 0..3 {
        pending.recv_timeout(Duration::from_secs(2))?;
    }
    work.send((
        1,
        Work::Git(muxy_protocol::GitRequest {
            project,
            action: muxy_protocol::GitAction::Summary,
        }),
    ))?;
    pending.recv_timeout(Duration::from_secs(2))?;
    for request in [
        Work::ReadCatalog,
        Work::ReadActivity,
        Work::ReadServerSettings,
        Work::ProjectSessions { project },
        Work::ReadSaved {
            pane: PaneId::new(),
            session: SessionId::from(std::num::NonZeroU64::MIN),
        },
        Work::Flush,
    ] {
        work.send((1, request))?;
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut received = [false; 6];
    while Instant::now() < deadline && !received.iter().all(|received| *received) {
        match updates.try_recv() {
            Ok((1, update)) => match update {
                Update::Catalog(_) => received[0] = true,
                Update::Activity(_) => received[1] = true,
                Update::ServerSettings(_) => received[2] = true,
                Update::ProjectSessions { .. } => received[3] = true,
                Update::Saved { .. } => received[4] = true,
                Update::Flushed => received[5] = true,
                _ => (),
            },
            _ => thread::sleep(Duration::from_millis(5)),
        }
    }
    work.send((1, Work::Stop))?;
    client.disconnect();
    drop((files, git, command));
    server.join().map_err(|_| "fake server panicked")??;
    assert!(
        received.iter().all(|received| *received),
        "app catalog, activity, settings, project sessions, saved screens and flush must complete while extension replies remain pending: {received:?}"
    );
    Ok(())
}

fn withhold_extension_replies(listener: &UnixListener, started: &Sender<()>) -> TestResult {
    let (socket, _) = listener.accept()?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut decoder = Decoder::new(socket.try_clone()?);
    let mut encoder = Encoder::new(socket);
    assert!(matches!(decoder.next()?, (CONTROL, Message::Hello { .. })));
    encoder.send(
        CONTROL,
        &Message::HelloReply {
            versions: SUPPORTED.to_vec(),
            server: muxy_protocol::ServerInfo::current(),
        },
    )?;
    identify_desktop(&mut decoder, &mut encoder)?;
    while let Ok((_, message)) = decoder.next() {
        let Message::Request { id, body } = message else {
            continue;
        };
        if matches!(
            body,
            RequestBody::Git(_) | RequestBody::Files(_) | RequestBody::Exec(_)
        ) {
            started.send(())?;
            continue;
        }
        let body = if matches!(body, RequestBody::ListSessions) {
            ReplyBody::Sessions(Vec::new())
        } else {
            ReplyBody::Error(muxy_protocol::ErrorReply {
                code: ErrorCode::BadRequest,
                message: "test response".into(),
            })
        };
        encoder.send(CONTROL, &Message::Reply { id, body })?;
    }
    Ok(())
}
