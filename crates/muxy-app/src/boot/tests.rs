use super::*;
use std::error::Error;

use muxy_protocol::{Message, ReplyBody};

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

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
            server_input: false,
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
