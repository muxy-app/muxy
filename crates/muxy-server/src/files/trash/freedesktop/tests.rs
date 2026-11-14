#![allow(
    clippy::unwrap_used,
    reason = "Test fixtures and assertions fail immediately"
)]
use super::*;
use crate::files::tests::Workspace;
use std::os::unix::fs::PermissionsExt;

#[test]
fn trash_rejects_symlinks_and_insecure_permissions() {
    let data = Workspace::new();
    let outside = Workspace::new();
    std::os::unix::fs::symlink(&outside.path, data.path.join("Trash")).unwrap();
    assert!(Trash::open(&data.path, OsStr::new("Trash"), None).is_err());
    std::fs::remove_file(data.path.join("Trash")).unwrap();
    std::fs::create_dir(data.path.join("Trash")).unwrap();
    std::fs::set_permissions(
        data.path.join("Trash"),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    assert!(Trash::open(&data.path, OsStr::new("Trash"), None).is_err());
    assert!(std::fs::read_dir(&outside.path).unwrap().next().is_none());
}

#[test]
fn volume_trash_uses_sticky_shared_directory_or_private_fallback() {
    let data = Workspace::new();
    let fallback = volume_trash(&data.path).unwrap();
    assert!(
        fallback
            .path
            .file_name()
            .unwrap()
            .as_bytes()
            .starts_with(b".Trash-")
    );
    std::fs::create_dir(data.path.join(".Trash")).unwrap();
    std::fs::set_permissions(
        data.path.join(".Trash"),
        std::fs::Permissions::from_mode(0o1777),
    )
    .unwrap();
    let shared = volume_trash(&data.path).unwrap();
    assert_eq!(
        shared.path.parent(),
        Some(data.path.join(".Trash").as_path())
    );
    std::fs::set_permissions(
        data.path.join(".Trash"),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    assert_eq!(volume_trash(&data.path).unwrap().path, fallback.path);
    assert!(
        mount_root(&data.path, std::fs::metadata(&data.path).unwrap().dev())
            .unwrap()
            .is_absolute()
    );
}

#[test]
fn deletion_date_is_the_local_time() {
    let minute = || {
        let output = std::process::Command::new("date")
            .arg("+%Y-%m-%dT%H:%M")
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    let before = minute();
    let date = deletion_date().unwrap();
    let after = minute();
    assert_eq!(date.len(), 19, "{date}");
    assert!(
        date.starts_with(&before) || date.starts_with(&after),
        "{date}"
    );
}
