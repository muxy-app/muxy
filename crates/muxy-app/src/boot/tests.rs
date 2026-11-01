use super::*;
use std::error::Error;

use muxy_protocol::wire::{Decoder, Encoder};
use muxy_protocol::{CONTROL, ErrorReply, Message, ReplyBody, SUPPORTED, ServerInfo};

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

#[test]
fn a_terminal_and_its_frames_pass_running_requests_while_other_events_wait() -> TestResult {
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 2;
    let pane = PaneId::new();
    delivery.start(&attach_work(7, pane));
    let catalog = ClientEvent::CatalogChanged { revision: 1 };
    assert!(delivery.event(catalog.clone())?.is_empty());
    assert!(matches!(
        delivery.event(attached_event(7, ChannelId(1))?)?.as_slice(),
        [Update::Attached { pane: attached, created: true, .. }] if *attached == pane
    ));
    assert!(matches!(
        delivery.event(frame_event(ChannelId(1)))?.as_slice(),
        [Update::Event(ClientEvent::Frame {
            channel: ChannelId(1),
            ..
        })]
    ));
    assert!(delivery.event(ClientEvent::Disconnected)?.is_empty());
    assert!(delivery.flush().is_empty());
    assert!(delivery.complete(None).is_empty());
    let ready = delivery.complete(None);
    assert!(matches!(
        ready.as_slice(),
        [Update::Event(event), Update::Event(ClientEvent::Disconnected), Update::Flushed] if *event == catalog
    ));
    Ok(())
}

#[test]
fn events_wait_for_a_terminal_whose_attach_returned_first() -> TestResult {
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 1;
    delivery.start(&attach_work(3, PaneId::new()));
    assert!(delivery.complete(None).is_empty());
    let session = SessionId::from(std::num::NonZeroU64::MIN);
    let ended = ClientEvent::SessionEnded {
        session,
        reason: muxy_protocol::ExitReason::Ended,
    };
    assert!(delivery.flush().is_empty());
    let ready = delivery.event(attached_event(3, ChannelId(1))?)?;
    assert!(matches!(
        ready.as_slice(),
        [Update::Attached { .. }, Update::Flushed]
    ));
    assert!(matches!(
        delivery.event(ended.clone())?.as_slice(),
        [Update::Event(event)] if *event == ended
    ));
    Ok(())
}

#[test]
fn a_session_that_ends_after_its_terminal_arrived_closes_while_requests_run() -> TestResult {
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 2;
    delivery.start(&attach_work(5, PaneId::new()));
    let session = SessionId::from(std::num::NonZeroU64::MIN);
    let ended = ClientEvent::SessionEnded {
        session,
        reason: muxy_protocol::ExitReason::Ended,
    };
    delivery.event(attached_event(5, ChannelId(1))?)?;
    assert!(matches!(
        delivery.event(ended.clone())?.as_slice(),
        [Update::Event(event)] if *event == ended
    ));
    Ok(())
}

#[test]
fn a_terminal_nobody_waits_for_is_detached_and_a_close_check_reads_its_process() -> TestResult {
    let mut delivery = delivery::Delivery::default();
    assert!(delivery.event(attached_event(9, ChannelId(4))?)?.is_empty());
    assert_eq!(delivery.take_strays(), [ChannelId(4)]);
    let check = CloseCheckId::new();
    delivery.pending = 1;
    delivery.start(&Work::CheckClose {
        check,
        session: SessionId::from(std::num::NonZeroU64::MIN),
        size: Size { cols: 80, rows: 24 },
    });
    assert!(matches!(
        delivery.event(attached_event(check.tag(), ChannelId(5))?)?.as_slice(),
        [Update::CloseChecked { check: checked, result: Ok(None), .. }] if *checked == check
    ));
    assert!(delivery.take_strays().is_empty());
    Ok(())
}

#[test]
fn a_failed_attach_stops_waiting_for_its_terminal() -> TestResult {
    let mut delivery = delivery::Delivery::default();
    let pane = PaneId::new();
    delivery.pending = 1;
    delivery.start(&attach_work(2, pane));
    delivery.complete(Some(Update::AttachFailed {
        pane,
        session: None,
        created: false,
        error: ClientError::Timeout,
    }));
    assert!(matches!(delivery.flush().as_slice(), [Update::Flushed]));
    assert!(delivery.event(attached_event(2, ChannelId(1))?)?.is_empty());
    assert_eq!(delivery.take_strays(), [ChannelId(1)]);
    Ok(())
}

fn attach_work(tag: u64, pane: PaneId) -> Work {
    Work::Attach {
        tag,
        pane,
        project: muxy_protocol::ProjectId::new(),
        session: None,
        directory: PathBuf::from("/tmp"),
        size: Size { cols: 80, rows: 24 },
        environment: BTreeMap::new(),
        colors: None,
    }
}

fn attached_event(
    tag: u64,
    channel: ChannelId,
) -> Result<ClientEvent, Box<dyn Error + Send + Sync>> {
    let Update::Attached {
        session,
        mut attachment,
        ..
    } = attachment_update(channel)?
    else {
        return Err("expected an attachment".into());
    };
    attachment.process = None;
    Ok(ClientEvent::Attached {
        tag,
        session,
        attachment: Box::new(attachment),
    })
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
            terminal: muxy_client::AttachedTerminal::new(
                channel,
                muxy_client::RunGrid::from_snapshot(&snapshot),
            ),
            server_input: false,
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
fn events_that_wait_fold_into_the_one_already_waiting_for_the_same_thing() -> TestResult {
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 1;
    let session = SessionId::from(std::num::NonZeroU64::MIN);
    let project = muxy_protocol::ProjectId::new();
    let progress = |percent| ClientEvent::Progress {
        session,
        progress: muxy_protocol::SessionProgress {
            progress: Some(muxy_protocol::TerminalProgress {
                state: muxy_protocol::ProgressState::Running,
                percent: Some(percent),
            }),
            completed: 0,
        },
    };
    let files = |path: &str| ClientEvent::FilesChanged {
        project,
        changes: muxy_protocol::FileChanges {
            paths: vec![muxy_protocol::ServerPath::from(path)],
            rescan: false,
        },
    };
    for event in [
        ClientEvent::SessionsChanged { revision: 4 },
        progress(10),
        files("a"),
        ClientEvent::GitChanged { project },
        ClientEvent::SessionsChanged { revision: 3 },
        progress(20),
        files("b"),
        ClientEvent::GitChanged { project },
        ClientEvent::CatalogChanged { revision: 1 },
    ] {
        assert!(delivery.event(event)?.is_empty());
    }
    let ready: Vec<_> = delivery
        .complete(None)
        .into_iter()
        .filter_map(|update| match update {
            Update::Event(event) => Some(event),
            _ => None,
        })
        .collect();
    let mut combined = files("a");
    combined.merge(files("b"));
    assert_eq!(
        ready,
        [
            ClientEvent::SessionsChanged { revision: 4 },
            progress(20),
            combined,
            ClientEvent::GitChanged { project },
            ClientEvent::CatalogChanged { revision: 1 },
        ]
    );
    Ok(())
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

#[test]
fn resizing_or_detaching_a_session_that_already_ended_shows_no_error() -> TestResult {
    let client = answering_every_request_with(ErrorReply {
        code: ErrorCode::UnknownSession,
        message: "session 7 does not exist".to_owned(),
    })?;
    let target = Target::Local(PathBuf::from("/nonexistent"));
    let size = Size { cols: 80, rows: 24 };
    assert!(perform(Work::Resize(ChannelId(3), size), &client, &target).is_none());
    assert!(perform(Work::Detach(ChannelId(3)), &client, &target).is_none());
    Ok(())
}

#[test]
fn terminal_requests_lost_with_the_connection_show_no_error() -> TestResult {
    let client = answering_every_request_with(ErrorReply {
        code: ErrorCode::BadSize,
        message: "bad size".to_owned(),
    })?;
    client.disconnect();
    let target = Target::Local(PathBuf::from("/nonexistent"));
    let size = Size { cols: 80, rows: 24 };
    assert!(perform(Work::Resize(ChannelId(3), size), &client, &target).is_none());
    assert!(perform(Work::Detach(ChannelId(3)), &client, &target).is_none());
    Ok(())
}

#[test]
fn a_resize_the_server_refuses_for_another_reason_still_shows_an_error() -> TestResult {
    let client = answering_every_request_with(ErrorReply {
        code: ErrorCode::BadSize,
        message: "bad size".to_owned(),
    })?;
    let target = Target::Local(PathBuf::from("/nonexistent"));
    let size = Size { cols: 80, rows: 24 };
    assert!(matches!(
        perform(Work::Resize(ChannelId(3), size), &client, &target),
        Some(Update::Error(_))
    ));
    Ok(())
}

/// A connected client whose server answers every request with `error`.
fn answering_every_request_with(error: ErrorReply) -> Result<Client, Box<dyn Error + Send + Sync>> {
    let (client, server) = muxy_protocol::transport::socket_pair()?;
    let mut decoder = Decoder::new(server.try_clone()?);
    let mut encoder = Encoder::new(server);
    thread::spawn(move || {
        if decoder.next().is_err() {
            return;
        }
        let hello = Message::HelloReply {
            versions: SUPPORTED.to_vec(),
            server: ServerInfo::current(),
            features: Vec::new(),
        };
        if encoder.send(CONTROL, &hello).is_err() {
            return;
        }
        while let Ok((_, Message::Request { id, .. })) = decoder.next() {
            let reply = Message::Reply {
                id,
                body: ReplyBody::Error(error.clone()),
            };
            if encoder.send(CONTROL, &reply).is_err() {
                return;
            }
        }
    });
    Ok(Client::from_stream(Box::new(client))?)
}
