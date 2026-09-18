use muxy_protocol::{
    CONTROL, FileChanges, FileContent, FileEntry, FileInfo, FilesAction, FilesReply, FilesRequest,
    MAX_FILE_BYTES, MAX_FILE_CHANGES, Message, ProjectId, ReplyBody, RequestBody, RequestId,
    ServerPath,
    wire::{Decoder, encode},
};

fn p(value: &str) -> ServerPath {
    ServerPath(value.as_bytes().to_vec())
}

#[test]
fn files_contract_round_trips_all_operations_and_results() -> Result<(), Box<dyn std::error::Error>>
{
    let project = ProjectId::new();
    let mut messages = Vec::new();
    for action in [
        FilesAction::List(p("")),
        FilesAction::Read(p("file")),
        FilesAction::Stat(p(".")),
        FilesAction::Write {
            path: p("file"),
            content: "Hello\n".into(),
        },
        FilesAction::Mkdir(p("folder")),
        FilesAction::Rename {
            path: p("file"),
            name: p("new"),
        },
        FilesAction::Move {
            paths: vec![p("new")],
            into: p("folder"),
        },
        FilesAction::Delete(vec![p("folder/new")]),
        FilesAction::Watch,
        FilesAction::Unwatch,
    ] {
        messages.push(Message::Request {
            id: RequestId(1),
            body: RequestBody::Files(FilesRequest { project, action }),
        });
    }
    for reply in [
        FilesReply::Entries(vec![FileEntry {
            name: p("file"),
            path: p("folder/file"),
            is_directory: false,
            is_ignored: true,
        }]),
        FilesReply::Content(FileContent {
            path: p("file"),
            content: "Hello\n".into(),
            size: 6,
        }),
        FilesReply::Info(FileInfo {
            name: p("folder"),
            path: p(""),
            is_directory: true,
            size: 0,
        }),
        FilesReply::Path(p("folder")),
        FilesReply::Paths(vec![p("folder/new")]),
        FilesReply::Done,
    ] {
        messages.push(Message::Reply {
            id: RequestId(1),
            body: ReplyBody::Files(reply),
        });
    }
    messages.push(Message::FilesChanged {
        project,
        changes: FileChanges {
            paths: vec![ServerPath(b"odd\xff\n".to_vec())],
            rescan: false,
        },
    });
    for message in messages {
        assert_eq!(message.validate(), Ok(()));
        let mut bytes = Vec::new();
        encode(&message, CONTROL, &mut bytes)?;
        assert_eq!(Decoder::new(bytes.as_slice()).next()?, (CONTROL, message));
    }
    Ok(())
}

#[test]
fn files_validation_bounds_content_names_selections_and_events() {
    for action in [
        FilesAction::Read(p("/absolute")),
        FilesAction::Stat(p("nul\0")),
        FilesAction::Write {
            path: p("file"),
            content: "a".repeat(MAX_FILE_BYTES + 1),
        },
        FilesAction::Rename {
            path: p("file"),
            name: p("../other"),
        },
        FilesAction::Rename {
            path: p("file"),
            name: p(".."),
        },
        FilesAction::Delete(vec![p("file"); 4097]),
        FilesAction::Read(p(&"a".repeat(4097))),
    ] {
        assert!(
            FilesRequest {
                project: ProjectId::new(),
                action
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        FilesReply::Content(FileContent {
            path: p("file"),
            content: "text".into(),
            size: 1
        })
        .validate()
        .is_err()
    );
    assert!(
        FileChanges {
            paths: vec![p("file"); MAX_FILE_CHANGES + 1],
            rescan: false
        }
        .validate()
        .is_err()
    );
}

#[test]
fn change_merging_deduplicates_and_converts_overflow_to_rescan() {
    let mut changes = FileChanges::default();
    for _ in 0..5 {
        changes.merge(&FileChanges {
            paths: vec![p("file")],
            rescan: false,
        });
    }
    assert_eq!(changes.paths, vec![p("file")]);
    for index in 0..MAX_FILE_CHANGES {
        changes.merge(&FileChanges {
            paths: vec![p(&index.to_string())],
            rescan: false,
        });
    }
    assert!(changes.rescan);
    assert!(changes.paths.is_empty());
    changes.merge(&FileChanges {
        paths: vec![p("later")],
        rescan: false,
    });
    assert!(changes.paths.is_empty());
}
