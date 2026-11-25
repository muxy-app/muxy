use super::*;
use muxy_protocol::ErrorCode;

#[test]
fn status_keeps_byte_paths_rename_sources_and_both_states() -> Result<()> {
    let parsed =
        files(b"\0RM new\tname\n\xff\0old\n\xfe\0?? untracked\0 C copy\0source\0UU conflict\0")?;
    let expected = [
        (&b"new\tname\n\xff"[..], Some(&b"old\n\xfe"[..]), b'R', b'M'),
        (&b"untracked"[..], None, b'?', b'?'),
        (&b"copy"[..], Some(&b"source"[..]), b' ', b'C'),
        (&b"conflict"[..], None, b'U', b'U'),
    ];
    assert_eq!(parsed.len(), expected.len());
    for (file, (path, original, index, worktree)) in parsed.iter().zip(expected) {
        assert_eq!(file.path.0, path);
        assert_eq!(
            file.original_path.as_ref().map(|path| path.0.as_slice()),
            original
        );
        assert_eq!((file.index, file.worktree), (index, worktree));
        assert_eq!((file.added, file.removed), (None, None));
    }
    Ok(())
}

#[test]
fn status_rejects_malformed_records_and_missing_rename_sources() {
    for (bytes, message) in [
        (&b"M\0"[..], "Invalid Git status"),
        (&b"MM?path\0"[..], "Invalid Git status"),
        (&b"R  renamed\0"[..], "Missing rename source"),
        (&b" C copied\0\0"[..], "Missing rename source"),
    ] {
        assert_eq!(files(bytes), Err(error(message)));
    }
}

#[test]
fn status_refuses_to_return_a_partial_list_past_the_file_limit() -> Result<()> {
    let mut bytes = b"?? path\0".repeat(4096);
    assert_eq!(files(&bytes)?.len(), 4096);
    bytes.extend_from_slice(b"?? overflow\0");
    assert_eq!(
        files(&bytes),
        Err(crate::ServerError::new(
            ErrorCode::BadRequest,
            "More than 4096 changed files; narrow changes using Git",
        ))
    );
    Ok(())
}
