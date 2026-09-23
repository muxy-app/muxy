use std::num::NonZeroU64;

use crate::{
    AttachSnapshot, ChannelId, Color, Cursor, ErrorCode, ErrorReply, ExitReason, HistoryCursor,
    HistoryPage, InputModes, Message, MetadataEvent, Modes, Modifiers, MouseAction, MouseButton,
    MouseEvent, ReplyBody, RequestBody, RequestId, Row, Run, ScreenFrame, SearchMatch, SearchPage,
    SearchSource, ServerPath, SessionId, Size, Style, TerminalColors, V1,
};

impl Message {
    pub fn samples() -> Vec<Self> {
        let session = SessionId::from(NonZeroU64::MIN);
        let channel = ChannelId(1);
        let size = Size { cols: 4, rows: 1 };
        let directory = ServerPath(b"/tmp".to_vec());
        let rows = vec![Row {
            index: 0,
            runs: vec![Run {
                text: "Muxy".to_owned(),
                width: 4,
                style: Style {
                    fg: Color::Rgb(80, 180, 240),
                    bg: Color::Indexed(0),
                    bold: true,
                    ..Style::default()
                },
            }],
        }];
        let cursor = sample_cursor();
        let modes = Modes {
            application_cursor_keys: true,
            bracketed_paste: true,
        };

        let mut samples = vec![
            Self::CellSize(crate::CellSize::default()),
            hello_sample(),
            Self::Request {
                id: RequestId(1),
                body: RequestBody::CreateSession {
                    project: crate::ProjectId::from_u128(1),
                    operation: crate::OperationId::from_u128(2),
                    directory: directory.clone(),
                    size,
                },
            },
            Self::FrameAck { channel, seq: 1 },
            hello_reply_sample(),
            Self::VersionUnsupported,
            Self::Reply {
                id: RequestId(2),
                body: ReplyBody::Attached {
                    snapshot: Box::new(AttachSnapshot {
                        graphics: crate::Graphics::default(),
                        prompts: vec![0],
                        channel,
                        size,
                        rows: rows.clone(),
                        cursor,
                        modes,
                        title: "Muxy".to_owned(),
                        directory,
                        history: Vec::new(),
                        history_cursor: None,
                        history_total: 0,
                    }),
                    process: None,
                },
            },
            Self::SessionEnded {
                session,
                reason: ExitReason::Exited(0),
            },
            Self::Fatal(ErrorReply {
                code: ErrorCode::BadRequest,
                message: "expected hello".to_owned(),
            }),
            Self::Input(b"pwd\r".to_vec()),
            Self::Frame(ScreenFrame {
                graphics: None,
                seq: 1,
                reset: true,
                rows,
                cursor,
                modes,
            }),
            Self::Metadata(MetadataEvent::Title("Muxy".to_owned())),
        ];
        let snapshot = samples.iter().find_map(|message| match message {
            Self::Reply {
                body: ReplyBody::Attached { snapshot, .. },
                ..
            } => Some(*snapshot.clone()),
            _ => None,
        });
        samples.extend(exec_samples());
        samples.extend(git_samples());
        samples.extend(git_review_samples());
        samples.extend(files_samples());
        samples.extend(snapshot.into_iter().flat_map(history_samples));
        samples.extend(input_samples());
        samples.extend(acknowledged_input_samples());
        samples.extend(search_samples(session, channel));
        samples.extend(color_samples());
        samples.extend(settings_samples());
        samples.extend(project_samples());
        samples.extend(close_samples());
        terminal_metadata_samples(&mut samples);
        samples.extend(activity_samples(session));
        samples
    }
}

fn search_samples(session: SessionId, channel: ChannelId) -> Vec<Message> {
    vec![
        Message::Request {
            id: RequestId(6),
            body: RequestBody::Search {
                source: SearchSource::Live(channel),
                query: "Muxy".into(),
                ignore_case: false,
                before: HistoryCursor(0),
                max_results: 500,
            },
        },
        Message::Request {
            id: RequestId(7),
            body: RequestBody::Search {
                source: SearchSource::Saved(session),
                query: "muxy".into(),
                ignore_case: true,
                before: HistoryCursor(42),
                max_results: 100,
            },
        },
        Message::Reply {
            id: RequestId(6),
            body: ReplyBody::SearchPage(SearchPage {
                matches: vec![SearchMatch {
                    row: 100,
                    start: 2,
                    end: 6,
                }],
                next: Some(HistoryCursor(42)),
                total_rows: 100,
                scanned_rows: 2000,
            }),
        },
    ]
}

fn history_samples(mut snapshot: AttachSnapshot) -> Vec<Message> {
    snapshot.history.clone_from(&snapshot.rows);
    snapshot.history_total = 2;
    snapshot.history_cursor = Some(HistoryCursor(42));
    vec![
        Message::Metadata(MetadataEvent::ScreenPrompts {
            seq: 1,
            rows: vec![0],
        }),
        Message::Metadata(MetadataEvent::History { total_rows: 5000 }),
        Message::Request {
            id: RequestId(3),
            body: RequestBody::HistoryPage {
                channel: snapshot.channel,
                before: HistoryCursor(42),
                max_rows: 500,
            },
        },
        Message::Request {
            id: RequestId(4),
            body: RequestBody::SavedHistoryPage {
                session: SessionId::from(NonZeroU64::MIN),
                before: HistoryCursor(0),
                max_rows: 200,
            },
        },
        Message::Reply {
            id: RequestId(3),
            body: ReplyBody::HistoryPage(HistoryPage {
                prompts: vec![0],
                rows: snapshot.history.clone(),
                next: None,
                total_rows: 2,
                screen: None,
            }),
        },
        Message::Reply {
            id: RequestId(5),
            body: ReplyBody::Attached {
                snapshot: Box::new(snapshot),
                process: None,
            },
        },
    ]
}

fn color_samples() -> [Message; 2] {
    [
        Message::Request {
            id: RequestId(8),
            body: RequestBody::SetTerminalColors(TerminalColors {
                foreground: [0xc9, 0xc2, 0xd9],
                background: [0x19, 0x17, 0x1f],
                cursor: [0xc3, 0x70, 0xd3],
                ansi: [[0x12, 0x34, 0x56]; 16],
                palette: std::collections::BTreeMap::from([(196, [0xab, 0xcd, 0xef])]),
                cursor_style: Some(crate::CursorShape::Bar),
                cursor_blink: Some(false),
            }),
        },
        Message::Reply {
            id: RequestId(8),
            body: ReplyBody::TerminalColorsSet,
        },
    ]
}

fn input_samples() -> [Message; 2] {
    [
        Message::Mouse(MouseEvent {
            action: MouseAction::Press,
            button: Some(MouseButton::Left),
            column: 3,
            row: 1,
            scroll: None,
            modifiers: Modifiers {
                shift: false,
                alt: true,
                ctrl: false,
            },
        }),
        Message::Metadata(MetadataEvent::InputModes(InputModes {
            mouse_tracking: true,
            alternate_scroll: false,
            focus_events: true,
        })),
    ]
}

fn settings_samples() -> Vec<Message> {
    let settings = crate::ServerSettingsDoc {
        default_shell: Some(ServerPath(b"/bin/bash".to_vec())),
        history_budget_bytes: 16 * 1024 * 1024,
        shell_integration: true,
    };
    let mut messages = vec![Message::ServerRestarting];
    for body in [
        RequestBody::ReadServerSettings,
        RequestBody::WriteServerSettings(settings.clone()),
        RequestBody::StopServer,
        RequestBody::StopServerIfIdle,
    ] {
        messages.push(Message::Request {
            id: RequestId(25),
            body,
        });
    }
    for body in [
        ReplyBody::ServerSettings(settings),
        ReplyBody::ServerSettingsWritten,
        ReplyBody::ServerStopping,
        ReplyBody::ServerBusy,
    ] {
        messages.push(Message::Reply {
            id: RequestId(25),
            body,
        });
    }
    messages
}

fn hello_reply_sample() -> Message {
    Message::HelloReply {
        versions: vec![V1],
        server: crate::ServerInfo {
            build: crate::BuildInfo {
                version: "fixture".into(),
                compatibility: crate::COMPATIBILITY,
            },
            instance: 1,
        },
    }
}

fn hello_sample() -> Message {
    Message::Hello {
        versions: vec![V1],
        compatibility: crate::COMPATIBILITY,
    }
}

fn sample_cursor() -> Cursor {
    Cursor {
        shape: crate::CursorShape::default(),
        row: 0,
        col: 3,
        visible: true,
    }
}

fn terminal_metadata_samples(samples: &mut Vec<Message>) {
    samples.push(Message::SessionMetadata {
        session: NonZeroU64::MIN.into(),
        metadata: crate::SessionMetadata {
            title: "Finished task".into(),
            directory: ServerPath(b"/tmp".to_vec()),
            process: None,
        },
    });
    samples.push(Message::Progress {
        session: SessionId::from(NonZeroU64::MIN),
        progress: crate::SessionProgress {
            progress: Some(crate::TerminalProgress {
                state: crate::ProgressState::Running,
                percent: Some(42),
            }),
            completed: 2,
        },
    });
    samples.push(Message::Metadata(MetadataEvent::CursorBlinking(true)));
    samples.push(Message::Metadata(MetadataEvent::Links {
        seq: 1,
        rows: vec![crate::LinkRow {
            row: 0,
            spans: vec![crate::LinkSpan {
                start: 0,
                end: 4,
                uri: "https://example.com".into(),
            }],
        }],
    }));
}

fn project_samples() -> Vec<Message> {
    use crate::{
        CatalogPage, OperationId, ProjectDescriptor, ProjectId, ProjectIntent, ProjectMutation,
        ProjectSession, ProjectSessions, ServerIdentity, SessionInfo, SessionStatus,
    };
    let project = ProjectDescriptor {
        id: ProjectId::from_u128(1),
        home: false,
        name: "Example".into(),
        directory: ServerPath(b"/tmp".to_vec()),
        icon: None,
        logo: None,
        color: "#808080".into(),
        kind: None,
        parent_id: None,
    };
    let operation = OperationId::from_u128(2);
    vec![
        Message::CatalogChanged { revision: 1 },
        Message::SessionsChanged { revision: 2 },
        Message::Request {
            id: RequestId(5),
            body: RequestBody::IdentifyClient(crate::ClientKind::Desktop),
        },
        Message::Reply {
            id: RequestId(5),
            body: ReplyBody::ClientIdentified(crate::SessionClient {
                id: crate::ClientId::from_u128(4),
                kind: crate::ClientKind::Desktop,
            }),
        },
        Message::Request {
            id: RequestId(1),
            body: RequestBody::ReadCatalog {
                after: None,
                revision: None,
            },
        },
        Message::Request {
            id: RequestId(2),
            body: RequestBody::MutateProject(ProjectIntent {
                operation,
                mutation: ProjectMutation::Create(project.clone()),
            }),
        },
        Message::Request {
            id: RequestId(3),
            body: RequestBody::ListProjectSessions {
                project: project.id,
                after: None,
                revision: None,
            },
        },
        Message::Request {
            id: RequestId(4),
            body: RequestBody::CancelCreation(operation),
        },
        Message::Reply {
            id: RequestId(1),
            body: ReplyBody::Catalog(CatalogPage {
                server: ServerIdentity::from_u128(3),
                home: project.id,
                revision: 1,
                projects: vec![ProjectDescriptor {
                    home: true,
                    ..project.clone()
                }],
                next: None,
                legacy_home: Some(project.id),
            }),
        },
        Message::Reply {
            id: RequestId(2),
            body: ReplyBody::ProjectMutated { revision: 1 },
        },
        Message::Reply {
            id: RequestId(3),
            body: ReplyBody::ProjectSessions(ProjectSessions {
                revision: 1,
                sessions: vec![ProjectSession {
                    info: SessionInfo {
                        id: SessionId::from(NonZeroU64::MIN),
                        project: project.id,
                        directory: project.directory,
                    },
                    status: SessionStatus::Live,
                    owner: Some(crate::SessionClient {
                        id: crate::ClientId::from_u128(4),
                        kind: crate::ClientKind::Desktop,
                    }),
                    attached: true,
                }],
                next: None,
            }),
        },
        Message::Reply {
            id: RequestId(4),
            body: ReplyBody::CreationCancelled,
        },
    ]
}

fn close_samples() -> Vec<Message> {
    let operation = crate::OperationId::from_u128(2);
    vec![
        Message::Request {
            id: RequestId(99),
            body: RequestBody::SyncSessionReferences {
                owner: Some(operation),
                revision: 1,
                sessions: vec![SessionId::from(NonZeroU64::MIN)],
            },
        },
        Message::Request {
            id: RequestId(99),
            body: RequestBody::CloseSession {
                session: SessionId::from(NonZeroU64::MIN),
                operation,
            },
        },
        Message::Reply {
            id: RequestId(99),
            body: ReplyBody::SessionReferencesSynced,
        },
        Message::Reply {
            id: RequestId(99),
            body: ReplyBody::SessionClosed,
        },
    ]
}

fn git_samples() -> Vec<Message> {
    vec![
        Message::GitChanged {
            project: crate::ProjectId::from_u128(1),
        },
        Message::Request {
            id: RequestId(100),
            body: RequestBody::Git(crate::GitRequest {
                project: crate::ProjectId::from_u128(1),
                action: crate::GitAction::Summary,
            }),
        },
        Message::Request {
            id: RequestId(101),
            body: RequestBody::Git(crate::GitRequest {
                project: crate::ProjectId::from_u128(1),
                action: crate::GitAction::BranchDiff {
                    base: "main".into(),
                    line_limit: Some(800),
                },
            }),
        },
        Message::Request {
            id: RequestId(103),
            body: RequestBody::Git(crate::GitRequest {
                project: crate::ProjectId::from_u128(1),
                action: crate::GitAction::PullRequest(crate::GitPullRequestAction::UpdateBranch {
                    number: 42,
                    expected_head: "def456".into(),
                }),
            }),
        },
        Message::Reply {
            id: RequestId(100),
            body: ReplyBody::Git(crate::GitReply::Changes(vec![crate::GitFile {
                path: ServerPath(b"file\xff".to_vec()),
                original_path: None,
                index: b'?',
                worktree: b'?',
                added: None,
                removed: None,
            }])),
        },
    ]
}

/// Samples for reviewing and applying AI-drafted commits and pull requests.
fn git_review_samples() -> Vec<Message> {
    let destination = crate::GitPushDestination {
        remote: "origin".into(),
        branch: "feature".into(),
    };
    vec![
        Message::Request {
            id: RequestId(102),
            body: RequestBody::Git(crate::GitRequest {
                project: crate::ProjectId::from_u128(1),
                action: crate::GitAction::ChangesPreview {
                    line_limit: Some(800),
                },
            }),
        },
        Message::Reply {
            id: RequestId(102),
            body: ReplyBody::Git(crate::GitReply::ChangesPreview(Box::new(
                crate::GitChangesPreview {
                    branch: Some("feature".into()),
                    head: Some("abc123".into()),
                    tree: "def456".into(),
                    diff: crate::GitRawDiff {
                        diff: "diff --git a/file b/file\n".into(),
                        truncated: false,
                    },
                    files: vec![crate::GitPreviewFile {
                        path: ServerPath(b"file".to_vec()),
                        added: Some(1),
                        removed: Some(0),
                        untracked: true,
                    }],
                    destination: Some(destination.clone()),
                },
            ))),
        },
        Message::Request {
            id: RequestId(104),
            body: RequestBody::Git(crate::GitRequest {
                project: crate::ProjectId::from_u128(1),
                action: crate::GitAction::CommitAll {
                    message: "Message".into(),
                    expected_head: Some("abc123".into()),
                    expected_tree: "def456".into(),
                },
            }),
        },
        Message::Request {
            id: RequestId(105),
            body: RequestBody::Git(crate::GitRequest {
                project: crate::ProjectId::from_u128(1),
                action: crate::GitAction::PublishBranch {
                    branch: "feature".into(),
                    destination,
                },
            }),
        },
        Message::Request {
            id: RequestId(106),
            body: RequestBody::Git(crate::GitRequest {
                project: crate::ProjectId::from_u128(1),
                action: crate::GitAction::SwitchToBase("main".into()),
            }),
        },
        Message::Reply {
            id: RequestId(106),
            body: ReplyBody::Git(crate::GitReply::BaseSwitch(
                crate::GitBaseSwitch::CheckedOutElsewhere(ServerPath(b"/tmp/main".to_vec())),
            )),
        },
    ]
}

fn activity_samples(session: SessionId) -> Vec<Message> {
    vec![
        Message::ActivityChanged { revision: 42 },
        Message::Request {
            id: RequestId(1),
            body: RequestBody::ReadActivity,
        },
        Message::Request {
            id: RequestId(2),
            body: RequestBody::AcknowledgeActivity(vec![1]),
        },
        Message::Request {
            id: RequestId(3),
            body: RequestBody::ClaimActivity(vec![1]),
        },
        Message::Reply {
            id: RequestId(1),
            body: ReplyBody::Activity(crate::ActivitySnapshot {
                revision: 42,
                agents: vec![crate::AgentActivity {
                    session,
                    project: crate::ProjectId::from_u128(1),
                    provider: crate::AgentProvider::Claude,
                    state: crate::AgentState::Blocked,
                }],
                events: vec![crate::ActivityEvent {
                    id: 1,
                    session,
                    project: crate::ProjectId::from_u128(1),
                    provider: crate::AgentProvider::Claude,
                    kind: crate::ActivityKind::Attention,
                    read: false,
                    timestamp: 123,
                }],
            }),
        },
        Message::Reply {
            id: RequestId(2),
            body: ReplyBody::ActivityAcknowledged,
        },
        Message::Reply {
            id: RequestId(3),
            body: ReplyBody::ActivityClaimed(vec![1]),
        },
    ]
}

fn files_samples() -> Vec<Message> {
    vec![
        Message::Request {
            id: RequestId(1),
            body: RequestBody::Files(crate::FilesRequest {
                project: crate::ProjectId::from_u128(1),
                action: crate::FilesAction::List(ServerPath(Vec::new())),
            }),
        },
        Message::Reply {
            id: RequestId(1),
            body: ReplyBody::Files(crate::FilesReply::Entries(vec![crate::FileEntry {
                name: ServerPath(b"file".to_vec()),
                path: ServerPath(b"file".to_vec()),
                is_directory: false,
                is_ignored: false,
            }])),
        },
        Message::FilesChanged {
            project: crate::ProjectId::from_u128(1),
            changes: crate::FileChanges {
                paths: vec![ServerPath(b"file".to_vec())],
                rescan: false,
            },
        },
    ]
}

fn acknowledged_input_samples() -> Vec<Message> {
    let channel = ChannelId(1);
    vec![
        Message::Request {
            id: RequestId(1),
            body: RequestBody::WriteInput {
                channel,
                bytes: b"pwd\r".to_vec(),
            },
        },
        Message::Reply {
            id: RequestId(1),
            body: ReplyBody::InputWritten,
        },
    ]
}

fn exec_samples() -> Vec<Message> {
    vec![
        Message::Request {
            id: RequestId(90),
            body: RequestBody::Exec(crate::ExecRequest {
                job: 1,
                project: crate::ProjectId::from_u128(1),
                argv: vec!["git".into(), "status".into()],
                shell: None,
                cwd: None,
                env: std::collections::BTreeMap::new(),
                stdin: Vec::new(),
                timeout_ms: 30000,
            }),
        },
        Message::Request {
            id: RequestId(91),
            body: RequestBody::CancelExec(1),
        },
        Message::Reply {
            id: RequestId(90),
            body: ReplyBody::Exec(crate::ExecResult {
                stdout: "output".into(),
                stderr: String::new(),
                exit_code: 0,
                timed_out: false,
                truncated: false,
                cancelled: false,
            }),
        },
        Message::Reply {
            id: RequestId(91),
            body: ReplyBody::ExecCancelled,
        },
    ]
}
