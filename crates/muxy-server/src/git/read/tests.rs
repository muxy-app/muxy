use super::*;
use crate::git::tests::Repository;
use muxy_protocol::ServerPath;
use std::collections::HashMap;

#[test]
fn line_stats_keep_renames_binary_files_and_staging_separate()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let repository = Repository::new();
    let path = &repository.path;
    run(path, &["config", "core.autocrlf", "false"])?;
    std::fs::write(path.join("old name.txt"), "one\ntwo\nthree\n")?;
    std::fs::write(path.join("data.bin"), b"\0old")?;
    run(path, &["add", "old name.txt", "data.bin"])?;
    run(
        path,
        &[
            "-c",
            "user.name=Muxy",
            "-c",
            "user.email=muxy@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "Files",
        ],
    )?;
    run(path, &["mv", "old name.txt", "new name.txt"])?;
    std::fs::write(path.join("data.bin"), b"\0new")?;
    run(path, &["add", "data.bin"])?;
    std::fs::write(path.join("new name.txt"), "one\ntwo\nthree\nfour\n")?;

    assert_eq!(
        line_stats(path, true)?,
        HashMap::from([
            (b"new name.txt".to_vec(), (Some(0), Some(0))),
            (b"data.bin".to_vec(), (None, None)),
        ])
    );
    assert_eq!(
        line_stats(path, false)?,
        HashMap::from([(b"new name.txt".to_vec(), (Some(1), Some(0)))])
    );
    let changes = changes(path)?;
    let renamed = changes
        .iter()
        .find(|file| file.path.0 == b"new name.txt")
        .ok_or("renamed file missing")?;
    assert_eq!(
        renamed.original_path,
        Some(ServerPath(b"old name.txt".to_vec()))
    );
    assert_eq!((renamed.added, renamed.removed), (Some(1), Some(0)));
    let binary = changes
        .iter()
        .find(|file| file.path.0 == b"data.bin")
        .ok_or("binary file missing")?;
    assert_eq!((binary.added, binary.removed), (None, None));
    Ok(())
}
