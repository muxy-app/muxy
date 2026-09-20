#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Test fixtures and assertions fail immediately"
)]

use super::*;
use muxy_protocol::{
    FileContent, MAX_FILE_BYTES, OperationId, ProjectDescriptor, ProjectId, ProjectIntent,
    ProjectMutation,
};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::sync::mpsc;

pub(super) struct Workspace {
    pub(super) path: PathBuf,
    registry: Registry,
    project: ProjectId,
}

impl Workspace {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("muxy-files-{}", OperationId::new()));
        std::fs::create_dir(&path).unwrap();
        let path = path.canonicalize().unwrap();
        let (send, _) = mpsc::channel();
        let registry = Registry::new(crate::ServerSettings::default(), send);
        let project = ProjectId::new();
        registry
            .mutate_project(&ProjectIntent {
                operation: OperationId::new(),
                mutation: ProjectMutation::Create(ProjectDescriptor {
                    id: project,
                    home: false,
                    directory: wire_path(&path),
                    name: "Test".into(),
                    icon: None,
                    color: "#ffffff".into(),
                    kind: None,
                    parent_id: None,
                }),
            })
            .unwrap();
        Self {
            path,
            registry,
            project,
        }
    }

    fn files(&self, action: FilesAction) -> Result<FilesReply> {
        self.registry.files(&FilesRequest {
            project: self.project,
            action,
        })
    }

    fn write(&self, name: &str, content: &str) -> Result<FilesReply> {
        self.files(FilesAction::Write {
            path: p(name),
            content: content.into(),
        })
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn p(value: &str) -> ServerPath {
    ServerPath(value.as_bytes().to_vec())
}

#[test]
fn atomic_utf8_writes_preserve_modes_and_enforce_both_limits() {
    let workspace = Workspace::new();
    assert_eq!(
        workspace.write("note", "Grüße\n").unwrap(),
        FilesReply::Path(p("note"))
    );
    assert_eq!(
        workspace.files(FilesAction::Read(p("note"))).unwrap(),
        FilesReply::Content(FileContent {
            path: p("note"),
            content: "Grüße\n".into(),
            size: 8
        })
    );
    std::fs::set_permissions(
        workspace.path.join("note"),
        std::fs::Permissions::from_mode(0o751),
    )
    .unwrap();
    workspace.write("note", "new").unwrap();
    assert_eq!(
        std::fs::metadata(workspace.path.join("note"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o751
    );
    let large = "a".repeat(MAX_FILE_BYTES);
    workspace.write("large", &large).unwrap();
    assert!(
        matches!(workspace.files(FilesAction::Read(p("large"))).unwrap(), FilesReply::Content(file) if file.content == large)
    );
    assert!(workspace.write("large", &(large + "b")).is_err());
    std::fs::write(workspace.path.join("huge"), vec![0; MAX_FILE_BYTES + 1]).unwrap();
    assert!(workspace.files(FilesAction::Read(p("huge"))).is_err());
    std::fs::write(workspace.path.join("binary"), [0xff, 0xfe]).unwrap();
    assert!(workspace.files(FilesAction::Read(p("binary"))).is_err());
    assert!(workspace.write("missing/child", "x").is_err());
    assert!(std::fs::read_dir(&workspace.path).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".muxy-write-")
    }));
}

#[test]
fn directories_sort_first_and_gitignore_flags_include_unusual_names() {
    let workspace = Workspace::new();
    assert!(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&workspace.path)
            .status()
            .unwrap()
            .success()
    );
    workspace.write(".gitignore", "*.log\nignored/\n").unwrap();
    workspace.write("a.log", "x").unwrap();
    workspace.write("-dash", "x").unwrap();
    workspace.write("line\nbreak", "x").unwrap();
    workspace.files(FilesAction::Mkdir(p("ignored"))).unwrap();
    let FilesReply::Entries(entries) = workspace.files(FilesAction::List(p(""))).unwrap() else {
        panic!();
    };
    assert_eq!(entries[0].name, p("ignored"));
    assert!(entries[0].is_directory && entries[0].is_ignored);
    assert!(
        entries
            .iter()
            .any(|entry| entry.name == p("a.log") && entry.is_ignored)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.name == p("-dash") && !entry.is_ignored)
    );
    assert!(!entries.iter().any(|entry| entry.name == p(".git")));
    assert!(workspace.files(FilesAction::List(p("a.log"))).is_err());
    assert!(workspace.files(FilesAction::List(p("missing"))).is_err());
}

#[test]
fn mkdir_rename_and_move_do_not_overwrite_collisions() {
    let workspace = Workspace::new();
    for name in ["first", "second", "archive"] {
        workspace.files(FilesAction::Mkdir(p(name))).unwrap();
    }
    assert_eq!(
        workspace.files(FilesAction::Mkdir(p("first"))).unwrap(),
        FilesReply::Path(p("first 2"))
    );
    workspace.write("first/report.txt", "first").unwrap();
    workspace.write("second/report.txt", "second").unwrap();
    workspace.write("archive/report.txt", "existing").unwrap();
    let reply = workspace
        .files(FilesAction::Move {
            paths: vec![p("first/report.txt"), p("second/report.txt")],
            into: p("archive"),
        })
        .unwrap();
    assert_eq!(
        reply,
        FilesReply::Paths(vec![p("archive/report 2.txt"), p("archive/report 3.txt")])
    );
    assert_eq!(
        std::fs::read_to_string(workspace.path.join("archive/report.txt")).unwrap(),
        "existing"
    );
    assert!(
        workspace
            .files(FilesAction::Rename {
                path: p("archive/report 2.txt"),
                name: p("report.txt")
            })
            .is_err()
    );
    assert_eq!(
        workspace
            .files(FilesAction::Rename {
                path: p("archive/report 2.txt"),
                name: p("renamed")
            })
            .unwrap(),
        FilesReply::Path(p("archive/renamed"))
    );
    assert!(
        workspace
            .files(FilesAction::Rename {
                path: p("missing"),
                name: p("missing")
            })
            .is_err()
    );
    assert!(
        workspace
            .files(FilesAction::Move {
                paths: vec![p("archive")],
                into: p("archive")
            })
            .is_err()
    );
    assert_eq!(
        workspace
            .files(FilesAction::Move {
                paths: vec![p("archive/renamed")],
                into: p("archive")
            })
            .unwrap(),
        FilesReply::Paths(vec![p("archive/renamed")])
    );
    assert_eq!(
        workspace
            .files(FilesAction::Move {
                paths: vec![p("archive/renamed")],
                into: p("")
            })
            .unwrap(),
        FilesReply::Paths(vec![p("renamed")])
    );
}

#[test]
fn root_aliases_and_traversal_cannot_be_mutated() {
    let workspace = Workspace::new();
    std::fs::create_dir(workspace.path.join("nested")).unwrap();
    symlink(".", workspace.path.join("root-link")).unwrap();
    for root in ["", ".", "nested/..", "root-link"] {
        assert!(
            matches!(workspace.files(FilesAction::Stat(p(root))).unwrap(), FilesReply::Info(info) if info.is_directory && info.path == p(""))
        );
        assert!(workspace.write(root, "x").is_err());
        assert!(workspace.files(FilesAction::Mkdir(p(root))).is_err());
        assert!(
            workspace
                .files(FilesAction::Rename {
                    path: p(root),
                    name: p("new")
                })
                .is_err()
        );
        assert!(
            workspace
                .files(FilesAction::Move {
                    paths: vec![p(root)],
                    into: p("nested")
                })
                .is_err()
        );
        assert!(workspace.files(FilesAction::Delete(vec![p(root)])).is_err());
    }
    for escape in ["../file", "nested/../../file", "/tmp/file"] {
        assert!(workspace.write(escape, "x").is_err());
        assert!(workspace.files(FilesAction::Read(p(escape))).is_err());
    }
}

#[test]
fn links_follow_internal_targets_and_reject_external_dangling_and_cyclic_targets() {
    let workspace = Workspace::new();
    let outside = Workspace::new();
    workspace.files(FilesAction::Mkdir(p("real"))).unwrap();
    symlink(workspace.path.join("real"), workspace.path.join("alias")).unwrap();
    assert_eq!(
        workspace.write("alias/../alias/note", "text").unwrap(),
        FilesReply::Path(p("real/note"))
    );
    symlink(&outside.path, workspace.path.join("external")).unwrap();
    symlink("external", workspace.path.join("hop")).unwrap();
    symlink(
        outside.path.join("missing"),
        workspace.path.join("dangling"),
    )
    .unwrap();
    symlink("cycle", workspace.path.join("cycle")).unwrap();
    for path in ["external/file", "hop/file", "dangling", "cycle"] {
        assert!(workspace.write(path, "no").is_err());
        assert!(workspace.files(FilesAction::Stat(p(path))).is_err());
        assert!(workspace.files(FilesAction::Delete(vec![p(path)])).is_err());
    }
    assert!(!outside.path.join("file").exists());
    assert!(!outside.path.join("missing").exists());
}

#[test]
fn batch_validation_precedes_mutation_and_rejects_overlaps() {
    let workspace = Workspace::new();
    workspace.write("keep", "original").unwrap();
    workspace.files(FilesAction::Mkdir(p("folder"))).unwrap();
    workspace.write("folder/file", "x").unwrap();
    for paths in [
        vec![p("keep"), p("../escape")],
        vec![p("keep"), p("missing")],
        vec![p("keep"), p("keep")],
        vec![p("folder"), p("folder/file")],
    ] {
        assert!(
            workspace
                .files(FilesAction::Move {
                    paths: paths.clone(),
                    into: p("folder")
                })
                .is_err()
        );
        assert!(workspace.files(FilesAction::Delete(paths)).is_err());
    }
    assert_eq!(
        std::fs::read_to_string(workspace.path.join("keep")).unwrap(),
        "original"
    );
}

#[test]
fn directory_writes_and_fifo_reads_fail_without_blocking() {
    let workspace = Workspace::new();
    workspace.files(FilesAction::Mkdir(p("folder"))).unwrap();
    assert!(workspace.write("folder", "no").is_err());
    assert!(
        std::process::Command::new("mkfifo")
            .arg(workspace.path.join("fifo"))
            .status()
            .unwrap()
            .success()
    );
    assert!(workspace.files(FilesAction::Read(p("fifo"))).is_err());
}

#[test]
fn concurrent_symlink_swaps_never_expose_external_content() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let workspace = Workspace::new();
    let outside = Workspace::new();
    workspace.files(FilesAction::Mkdir(p("folder"))).unwrap();
    workspace.write("folder/file", "inside").unwrap();
    outside.write("file", "SECRET").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&stop);
    let directory = workspace.path.clone();
    let external = outside.path.clone();
    let task = std::thread::spawn(move || {
        while !done.load(Ordering::Relaxed) {
            std::fs::rename(directory.join("folder"), directory.join("saved")).unwrap();
            symlink(&external, directory.join("folder")).unwrap();
            std::fs::remove_file(directory.join("folder")).unwrap();
            std::fs::rename(directory.join("saved"), directory.join("folder")).unwrap();
        }
    });
    for _ in 0..300 {
        if let Ok(FilesReply::Content(file)) = workspace.files(FilesAction::Read(p("folder/file")))
        {
            assert_eq!(file.content, "inside");
        }
    }
    stop.store(true, Ordering::Relaxed);
    task.join().unwrap();
}

#[test]
fn unix_paths_are_lossless() {
    let workspace = Workspace::new();
    #[cfg(target_os = "linux")]
    let name = ServerPath(b"odd\xff\n[*]".to_vec());
    #[cfg(not(target_os = "linux"))]
    let name = p("odd\n[*]");
    workspace
        .files(FilesAction::Write {
            path: name.clone(),
            content: "x".into(),
        })
        .unwrap();
    assert!(
        matches!(workspace.files(FilesAction::List(p(""))).unwrap(), FilesReply::Entries(entries) if entries[0].name == name)
    );
    assert!(
        matches!(workspace.files(FilesAction::Read(name.clone())).unwrap(), FilesReply::Content(content) if content.path == name)
    );
}

#[test]
fn unknown_projects_and_direct_subscriptions_return_errors() {
    let workspace = Workspace::new();
    assert_eq!(
        workspace
            .registry
            .files(&FilesRequest {
                project: ProjectId::new(),
                action: FilesAction::Stat(p(""))
            })
            .unwrap_err()
            .code(),
        ErrorCode::UnknownProject
    );
    assert!(workspace.files(FilesAction::Watch).is_err());
}

#[test]
fn project_reads_overlap_and_mutations_wait_for_readers() {
    let workspace = Workspace::new();
    let lock = workspace.registry.files.lock_for(&workspace.path);
    let guard = lock.read().unwrap();
    let workspace = &workspace;
    std::thread::scope(|scope| {
        let (read, completed) = mpsc::channel();
        scope.spawn(move || {
            let _ = read.send(workspace.registry.files(&FilesRequest {
                project: workspace.project,
                action: FilesAction::List(ServerPath(Vec::new())),
            }));
        });
        let result = completed.recv_timeout(std::time::Duration::from_secs(3));
        let (write, written) = mpsc::channel();
        scope.spawn(move || {
            let _ = write.send(workspace.registry.files(&FilesRequest {
                project: workspace.project,
                action: FilesAction::Mkdir(ServerPath(b"parallel".to_vec())),
            }));
        });
        let early_write = written.recv_timeout(std::time::Duration::from_millis(100));
        drop(guard);
        assert!(matches!(result.unwrap().unwrap(), FilesReply::Entries(_)));
        assert!(early_write.is_err());
        assert!(matches!(
            written
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
                .unwrap(),
            FilesReply::Path(_)
        ));
    });
}
