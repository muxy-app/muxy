use muxy_protocol::{
    FileBytes, FileChanges, FileContent, FilesAction, FilesReply, FilesRequest, MAX_FILE_BYTES,
    MAX_FILE_CHANGES, ProjectId, ServerPath,
};

fn p(value: &str) -> ServerPath {
    ServerPath(value.as_bytes().to_vec())
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
        FilesAction::ReadBytes(p("/absolute")),
        FilesAction::WriteBytes {
            path: p("file"),
            bytes: vec![0; MAX_FILE_BYTES + 1],
        },
        FilesAction::WriteBytes {
            path: p("/absolute"),
            bytes: Vec::new(),
        },
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
    for bytes in [
        FileBytes {
            path: p("file"),
            bytes: vec![0; MAX_FILE_BYTES + 1],
        },
        FileBytes {
            path: p("/absolute"),
            bytes: Vec::new(),
        },
    ] {
        assert!(FilesReply::Bytes(bytes).validate().is_err());
    }
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

#[test]
fn folder_listings_need_an_absolute_path_and_return_single_names() {
    use muxy_protocol::{
        ErrorCode, MAX_FILE_ENTRIES, ServerPath, validate_folder_names, validate_folder_path,
    };
    let path = |bytes: &[u8]| ServerPath(bytes.to_vec());
    assert_eq!(validate_folder_path(&path(b"/home/dev")), Ok(()));
    assert_eq!(validate_folder_path(&path(b"/")), Ok(()));
    for bad in [&b"home/dev"[..], b"", b"/home/\0dev"] {
        assert_eq!(validate_folder_path(&path(bad)), Err(ErrorCode::BadPath));
    }
    assert_eq!(
        validate_folder_path(&ServerPath(vec![b'/'; 4097])),
        Err(ErrorCode::BadPath)
    );
    assert_eq!(
        validate_folder_names(&[path(b"code"), path(b".config")]),
        Ok(())
    );
    for bad in [&b""[..], b"a/b", b".", b"..", b"a\0"] {
        assert_eq!(validate_folder_names(&[path(bad)]), Err(ErrorCode::BadPath));
    }
    let many = vec![path(b"a"); MAX_FILE_ENTRIES + 1];
    assert_eq!(validate_folder_names(&many), Err(ErrorCode::BadRequest));
}

#[test]
fn uploads_come_in_bounded_chunks_and_reply_with_an_absolute_path() {
    use muxy_protocol::{
        ErrorCode, MAX_UPLOAD_BYTES, MAX_UPLOAD_CHUNK, MAX_UPLOAD_NAME, OperationId, ServerPath,
        SessionId, UploadChunk, validate_uploaded,
    };
    let chunk = |offset: u64, length: usize, name: &str| UploadChunk {
        session: SessionId::new(1).expect("session"),
        upload: OperationId::new(),
        name: name.into(),
        offset,
        bytes: vec![0; length],
        last: false,
    };
    assert_eq!(chunk(0, MAX_UPLOAD_CHUNK, "shot.png").validate(), Ok(()));
    let last = MAX_UPLOAD_BYTES - MAX_UPLOAD_CHUNK as u64;
    assert_eq!(chunk(last, MAX_UPLOAD_CHUNK, "a").validate(), Ok(()));
    for bad in [
        chunk(0, MAX_UPLOAD_CHUNK + 1, "a"),
        chunk(last + 1, MAX_UPLOAD_CHUNK, "a"),
        chunk(u64::MAX, 1, "a"),
        chunk(0, 1, &"a".repeat(MAX_UPLOAD_NAME + 1)),
    ] {
        assert_eq!(bad.validate(), Err(ErrorCode::BadRequest));
    }
    let path = |bytes: &[u8]| ServerPath(bytes.to_vec());
    assert_eq!(validate_uploaded(None), Ok(()));
    assert_eq!(validate_uploaded(Some(&path(b"/srv/u/shot.png"))), Ok(()));
    for bad in [&b"relative.png"[..], b"/a\0b", b""] {
        assert_eq!(validate_uploaded(Some(&path(bad))), Err(ErrorCode::BadPath));
    }
}
