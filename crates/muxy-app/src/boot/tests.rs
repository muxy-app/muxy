use super::*;
use std::error::Error;

use muxy_protocol::{Message, ReplyBody};

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
