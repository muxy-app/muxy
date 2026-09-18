use std::error::Error;
use std::fs;
use std::path::PathBuf;

use muxy_protocol::wire::{Decoder, MessageKind, WireError, encode, message_version};
use muxy_protocol::{
    CONTROL, ChannelId, ChannelKind, Message, MetadataEvent, ReplyBody, RequestBody, SearchSource,
};

#[test]
fn golden_frames_match_byte_for_byte() -> Result<(), Box<dyn Error>> {
    for message in Message::samples() {
        let fixture = fixture_path(&message);
        let expected = fs::read(&fixture)?;
        let mut actual = Vec::new();
        encode(&message, channel(&message), &mut actual)?;
        assert_eq!(actual, expected, "{}", fixture.display());
        let mut decoder = Decoder::new(expected.as_slice());
        assert_eq!(decoder.next()?, (channel(&message), message));
    }
    Ok(())
}

#[test]
#[ignore = "regenerates the mutable development schema fixtures"]
fn generate_fixtures() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"))?;
    for message in Message::samples() {
        let mut bytes = Vec::new();
        encode(&message, channel(&message), &mut bytes)?;
        let path = fixture_path(&message);
        fs::write(path, bytes)?;
    }
    Ok(())
}

fn fixture_path(message: &Message) -> PathBuf {
    let name = project_fixture_name(message).unwrap_or_else(|| legacy_fixture_name(message));
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.bin"))
}

fn project_fixture_name(message: &Message) -> Option<&'static str> {
    Some(match message {
        Message::Request {
            body: RequestBody::ReadActivity,
            ..
        } => "read_activity",
        Message::Request {
            body: RequestBody::AcknowledgeActivity(_),
            ..
        } => "acknowledge_activity",
        Message::Request {
            body: RequestBody::ClaimActivity(_),
            ..
        } => "claim_activity",
        Message::Reply {
            body: ReplyBody::Activity(_),
            ..
        } => "activity_snapshot",
        Message::Reply {
            body: ReplyBody::ActivityAcknowledged,
            ..
        } => "activity_acknowledged",
        Message::Reply {
            body: ReplyBody::ActivityClaimed(_),
            ..
        } => "activity_claimed",
        Message::Request {
            body: RequestBody::Git(_),
            ..
        } => "git_request",
        Message::Reply {
            body: ReplyBody::Git(_),
            ..
        } => "git_reply",
        Message::SessionsChanged { .. } => "sessions_changed",
        Message::Request {
            body: RequestBody::IdentifyClient(_),
            ..
        } => "identify_client",
        Message::Reply {
            body: ReplyBody::ClientIdentified(_),
            ..
        } => "client_identified",
        Message::Request {
            body: RequestBody::SyncSessionReferences { .. },
            ..
        } => "session_references",
        Message::Request {
            body: RequestBody::CloseSession { .. },
            ..
        } => "close_session",
        Message::Reply {
            body: ReplyBody::SessionReferencesSynced,
            ..
        } => "session_references_synced",
        Message::Reply {
            body: ReplyBody::SessionClosed,
            ..
        } => "session_closed",
        Message::Request {
            body: RequestBody::ReadCatalog { .. },
            ..
        } => "ReadCatalog",
        Message::Request {
            body: RequestBody::MutateProject(_),
            ..
        } => "MutateProject",
        Message::Request {
            body: RequestBody::ListProjectSessions { .. },
            ..
        } => "ListProjectSessions",
        Message::Request {
            body: RequestBody::CancelCreation(_),
            ..
        } => "CancelCreation",
        Message::Reply {
            body: ReplyBody::Catalog(_),
            ..
        } => "Catalog",
        Message::Reply {
            body: ReplyBody::ProjectMutated { .. },
            ..
        } => "ProjectMutated",
        Message::Reply {
            body: ReplyBody::ProjectSessions(_),
            ..
        } => "ProjectSessions",
        Message::Reply {
            body: ReplyBody::CreationCancelled,
            ..
        } => "CreationCancelled",

        _ => return None,
    })
}

fn legacy_fixture_name(message: &Message) -> &'static str {
    match message {
        Message::FilesChanged { .. } => "files_changed",
        Message::Request {
            body: RequestBody::Files(_),
            ..
        } => "files_request",
        Message::Reply {
            body: ReplyBody::Files(_),
            ..
        } => "files_reply",
        Message::Request {
            body: RequestBody::StopServerIfIdle,
            ..
        } => "stop_server_if_idle",
        Message::Reply {
            body: ReplyBody::ServerBusy,
            ..
        } => "server_busy",
        Message::Request {
            body: RequestBody::ReadServerSettings,
            ..
        } => "read_server_settings",
        Message::Request {
            body: RequestBody::WriteServerSettings(_),
            ..
        } => "write_server_settings",
        Message::Request {
            body: RequestBody::StopServer,
            ..
        } => "stop_server",
        Message::Reply {
            body: ReplyBody::ServerSettings(_),
            ..
        } => "server_settings",
        Message::Reply {
            body: ReplyBody::ServerSettingsWritten,
            ..
        } => "server_settings_written",
        Message::Reply {
            body: ReplyBody::ServerStopping,
            ..
        } => "server_stopping",
        Message::Request {
            body: RequestBody::SetTerminalColors(_),
            ..
        } => "terminal_colors_request",
        Message::Reply {
            body: ReplyBody::TerminalColorsSet,
            ..
        } => "terminal_colors_reply",
        Message::Request {
            body:
                RequestBody::Search {
                    source: SearchSource::Live(_),
                    ..
                },
            ..
        } => "search_request",
        Message::Request {
            body:
                RequestBody::Search {
                    source: SearchSource::Saved(_),
                    ..
                },
            ..
        } => "saved_search_request",
        Message::Reply {
            body: ReplyBody::SearchPage(_),
            ..
        } => "search_reply",
        Message::Metadata(MetadataEvent::ScreenPrompts { .. }) => "screen_prompts",
        Message::Metadata(MetadataEvent::Links { .. }) => "links_metadata",
        Message::Metadata(MetadataEvent::InputModes(_)) => "input_modes",
        Message::Metadata(MetadataEvent::CursorBlinking(_)) => "cursor_blinking",
        Message::Metadata(MetadataEvent::History { .. }) => "history_metadata",
        Message::Progress { .. } => "session_progress",
        Message::Request {
            body: RequestBody::HistoryPage { .. },
            ..
        } => "history_request",
        Message::Request {
            body: RequestBody::SavedHistoryPage { .. },
            ..
        } => "saved_history_request",
        Message::Reply {
            body: ReplyBody::HistoryPage(_),
            ..
        } => "history_reply",
        Message::Reply {
            body: ReplyBody::Attached { snapshot, .. },
            ..
        } if !snapshot.history.is_empty() => "attached_history_reply",
        _ => kind_name(MessageKind::from(message)),
    }
}

fn channel(message: &Message) -> ChannelId {
    match message.channel_kind() {
        ChannelKind::Control => CONTROL,
        ChannelKind::Session => ChannelId(1),
    }
}

#[test]
fn development_messages_share_one_version_and_reject_unknown_schemas() -> Result<(), Box<dyn Error>>
{
    for message in Message::samples() {
        let mut bytes = Vec::new();
        encode(&message, channel(&message), &mut bytes)?;
        assert_eq!(message_version(&message), muxy_protocol::V1);
        assert_eq!(bytes[4..6], 1_u16.to_le_bytes());
        for version in [0_u16, 2, 9, u16::MAX] {
            bytes[4..6].copy_from_slice(&version.to_le_bytes());
            assert!(matches!(
                Decoder::new(bytes.as_slice()).next(),
                Err(WireError::UnsupportedVersion(_))
            ));
        }
    }
    Ok(())
}

fn kind_name(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::GitChanged => "git_changed",
        MessageKind::FilesChanged => "files_changed",
        MessageKind::Progress => "session_progress",
        MessageKind::SessionsChanged => "sessions_changed",
        MessageKind::CatalogChanged => "catalog_changed",
        MessageKind::Hello => "hello",
        MessageKind::Request => "request",
        MessageKind::FrameAck => "frame_ack",
        MessageKind::HelloReply => "hello_reply",
        MessageKind::ServerRestarting => "server_restarting",
        MessageKind::VersionUnsupported => "version_unsupported",
        MessageKind::Reply => "reply",
        MessageKind::SessionEnded => "session_ended",
        MessageKind::Fatal => "fatal",
        MessageKind::Input => "input",
        MessageKind::Frame => "frame",
        MessageKind::Metadata => "metadata",
        MessageKind::Mouse => "mouse",
        MessageKind::ActivityChanged => "activity_changed",
        MessageKind::SessionMetadata => "session_metadata",
        MessageKind::CellSize => "cell_size",
    }
}
