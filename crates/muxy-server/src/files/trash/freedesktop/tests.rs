#![allow(
    clippy::unwrap_used,
    reason = "Test fixtures and assertions fail immediately"
)]
use super::*;
use crate::files::tests::Workspace;
use std::os::unix::fs::PermissionsExt;

#[test]
fn trash_preserves_content_and_writes_recovery_metadata() {
    let workspace = Workspace::new();
    let data = Workspace::new();
    let trash = Trash::open(&data.path, OsStr::new("Trash"), None).unwrap();
    let root = Root::open(&workspace.path).unwrap();
    for content in ["first", "second"] {
        std::fs::write(workspace.path.join("a %\n.txt"), content).unwrap();
        let source = root.mutation(Path::new("a %\n.txt")).unwrap();
        trash
            .put(&source, &workspace.path.join("a %\n.txt"))
            .unwrap();
    }
    assert!(!workspace.path.join("a %\n.txt").exists());
    let mut contents = Vec::new();
    for entry in std::fs::read_dir(data.path.join("Trash/files")).unwrap() {
        let entry = entry.unwrap();
        contents.push(std::fs::read_to_string(entry.path()).unwrap());
        let info = std::fs::read_to_string(
            data.path
                .join("Trash/info")
                .join(format!("{}.trashinfo", entry.file_name().to_string_lossy())),
        )
        .unwrap();
        assert!(info.starts_with("[Trash Info]\nPath=/"));
        assert!(info.contains("a%20%25%0A.txt\nDeletionDate="));
    }
    contents.sort();
    assert_eq!(contents, ["first", "second"]);
}

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
fn failed_moves_leave_sources_intact_and_remove_only_the_new_info() {
    let workspace = Workspace::new();
    let data = Workspace::new();
    let trash = Trash::open(&data.path, OsStr::new("Trash"), None).unwrap();
    let root = Root::open(&workspace.path).unwrap();
    let source = root.mutation(Path::new("missing")).unwrap();
    assert!(trash.put(&source, &workspace.path.join("missing")).is_err());
    assert!(
        std::fs::read_dir(data.path.join("Trash/info"))
            .unwrap()
            .next()
            .is_none()
    );
    assert!(
        std::fs::read_dir(data.path.join("Trash/files"))
            .unwrap()
            .next()
            .is_none()
    );
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
