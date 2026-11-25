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
#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};
use std::sync::{Arc, mpsc};

pub(super) struct Workspace {
    pub(super) path: PathBuf,
    registry: Registry,
    project: ProjectId,
}

impl Workspace {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("muxy-files-{}", OperationId::new()));
        std::fs::create_dir(&path).unwrap();
        let path = muxy_core::files::canonicalize(&path).unwrap();
        let (send, _) = mpsc::channel();
        let registry = Registry::new(crate::ServerSettings::default(), send);
        let project = ProjectId::new();
        registry
            .mutate_project(&ProjectIntent {
                operation: OperationId::new(),
                mutation: ProjectMutation::Create(ProjectDescriptor {
                    id: project,
                    home: false,
                    directory: ServerPath::from_path(&path).unwrap(),
                    name: "Test".into(),
                    icon: None,
                    logo: None,
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
    ServerPath::from(value)
}

#[test]
fn file_locks_are_shared_by_key_until_the_last_caller_leaves() {
    let files = Files::default();
    let barrier = std::sync::Barrier::new(4);
    let locks = std::thread::scope(|scope| {
        let callers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    files.locks.get(PathBuf::from("project"))
                })
            })
            .collect();
        callers
            .into_iter()
            .map(|caller| caller.join().unwrap())
            .collect::<Vec<_>>()
    });
    let other = files.locks.get(PathBuf::from("other"));
    let writer = locks[0].write().unwrap();
    for lock in &locks {
        assert!(lock.try_read().is_err());
        assert!(lock.try_write().is_err());
    }
    assert!(other.try_write().is_ok());
    drop(writer);
    let reader = locks[0].read().unwrap();
    assert!(locks[1].try_read().is_ok());
    assert!(locks[1].try_write().is_err());
    drop(reader);
    let released = Arc::downgrade(&locks[0]);
    drop(locks);
    assert!(released.upgrade().is_none());
    assert!(
        files
            .locks
            .get(PathBuf::from("project"))
            .try_write()
            .is_ok()
    );
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
    #[cfg(unix)]
    {
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
    }
    #[cfg(windows)]
    {
        let open = std::fs::File::open(workspace.path.join("note")).unwrap();
        workspace.write("note", "new").unwrap();
        drop(open);
        assert_eq!(
            std::fs::read_to_string(workspace.path.join("note")).unwrap(),
            "new"
        );
    }
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
fn a_replacement_is_private_while_its_content_is_written() {
    let workspace = Workspace::new();
    let directory = native::open_root(&workspace.path).unwrap();
    let replacement =
        native::create_file(&directory, std::ffi::OsStr::new("replacement"), true).unwrap();
    #[cfg(unix)]
    {
        let mode = replacement.metadata().unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    #[cfg(windows)]
    {
        let opened = std::fs::File::open(workspace.path.join("replacement"));
        assert!(opened.is_err(), "another handle opened the replacement");
    }
    drop(replacement);
}

#[cfg(windows)]
#[test]
fn windows_writes_keep_a_files_own_dacl() {
    let workspace = Workspace::new();
    workspace.write("secret", "old").unwrap();
    let path = workspace.path.join("secret");
    let icacls = |args: &[&str]| {
        let output = std::process::Command::new("icacls")
            .arg(&path)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    let user = std::env::var("USERNAME").unwrap();
    icacls(&["/inheritance:r", "/grant:r", &format!("{user}:F")]);
    let before = icacls(&[]);
    workspace.write("secret", "new").unwrap();
    assert_eq!(icacls(&[]), before);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
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
fn mkdir_and_move_skip_mixed_collisions_and_use_the_first_available_name() {
    let workspace = Workspace::new();
    for directory in ["source", "folders", "archive"] {
        std::fs::create_dir(workspace.path.join(directory)).unwrap();
    }
    for (name, second, third, fourth) in [
        ("folder", "folder 2", "folder 3", "folder 4"),
        (".env", ".env 2", ".env 3", ".env 4"),
        (
            "résumé.tar.gz",
            "résumé.tar 2.gz",
            "résumé.tar 3.gz",
            "résumé.tar 4.gz",
        ),
    ] {
        for directory in ["folders", "archive"] {
            let directory = workspace.path.join(directory);
            std::fs::write(directory.join(name), "original").unwrap();
            std::fs::create_dir(directory.join(second)).unwrap();
            std::fs::write(directory.join(second).join("kept"), "nested").unwrap();
            std::fs::write(directory.join(fourth), "later collision").unwrap();
        }
        assert_eq!(
            workspace
                .files(FilesAction::Mkdir(p(&format!("folders/{name}"))))
                .unwrap(),
            FilesReply::Path(p(&format!("folders/{third}")))
        );
        assert!(workspace.path.join("folders").join(third).is_dir());
        std::fs::write(workspace.path.join("source").join(name), "moved").unwrap();
        assert_eq!(
            workspace
                .files(FilesAction::Move {
                    paths: vec![p(&format!("source/{name}"))],
                    into: p("archive"),
                })
                .unwrap(),
            FilesReply::Paths(vec![p(&format!("archive/{third}"))])
        );
        assert!(!workspace.path.join("source").join(name).exists());
        assert_eq!(
            std::fs::read_to_string(workspace.path.join("archive").join(third)).unwrap(),
            "moved"
        );
        for directory in ["folders", "archive"] {
            let directory = workspace.path.join(directory);
            for (path, content) in [
                (directory.join(name), "original"),
                (directory.join(second).join("kept"), "nested"),
                (directory.join(fourth), "later collision"),
            ] {
                assert_eq!(std::fs::read_to_string(path).unwrap(), content);
            }
        }
    }
}

#[test]
fn root_aliases_and_traversal_cannot_be_mutated() {
    let workspace = Workspace::new();
    std::fs::create_dir(workspace.path.join("nested")).unwrap();
    #[cfg(unix)]
    symlink(".", workspace.path.join("root-link")).unwrap();
    let roots: &[&str] = if cfg!(unix) {
        &["", ".", "nested/..", "root-link"]
    } else {
        &["", ".", "nested/..", "nested\\.."]
    };
    for &root in roots {
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
    let mut escapes = vec!["../file", "nested/../../file", "/tmp/file"];
    if cfg!(windows) {
        escapes.extend(["C:/file", "C:file", "\\\\?\\C:\\file", "..\\file"]);
    }
    for escape in escapes {
        assert!(workspace.write(escape, "x").is_err());
        assert!(workspace.files(FilesAction::Read(p(escape))).is_err());
    }
}

#[cfg(unix)]
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
fn connections_watching_one_project_share_its_watch_and_each_hear_changes() {
    let workspace = Workspace::new();
    let request = FilesRequest {
        project: workspace.project,
        action: FilesAction::Watch,
    };
    let connections: Vec<_> = (0..2)
        .map(|_| {
            let connection = Arc::new(crate::connection::Connection::new(Arc::default()));
            let mut subscriptions = watch::Subscriptions::default();
            subscriptions
                .request(&workspace.registry, &connection, &request)
                .unwrap();
            (connection, subscriptions)
        })
        .collect();
    assert_eq!(workspace.registry.files.watches.watching(), 1);

    std::fs::write(workspace.path.join("added.txt"), "new").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut heard = [false; 2];
    while heard.contains(&false) && std::time::Instant::now() < deadline {
        for ((connection, _), heard) in connections.iter().zip(&mut heard) {
            *heard |= connection.take_control().iter().any(|message| {
                matches!(message, muxy_protocol::Message::FilesChanged { changes, .. }
                    if changes.rescan || changes.paths.contains(&p("added.txt")))
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(heard, [true; 2]);
    drop(connections);
    assert_eq!(workspace.registry.files.watches.watching(), 0);
}

#[cfg(windows)]
#[test]
fn windows_refuses_junctions_links_streams_devices_and_trailing_dots() {
    let workspace = Workspace::new();
    let outside = Workspace::new();
    outside.write("secret", "SECRET").unwrap();
    workspace.files(FilesAction::Mkdir(p("real"))).unwrap();
    let junction = std::process::Command::new("cmd")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(workspace.path.join("junction"))
        .arg(&outside.path)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(junction.success());
    let mut refused = vec!["junction/secret", "junction"];
    let link =
        std::os::windows::fs::symlink_dir(workspace.path.join("real"), workspace.path.join("link"));
    if !link
        .as_ref()
        .is_err_and(|error| error.raw_os_error() == Some(1314))
    {
        link.unwrap();
        refused.push("link/file");
    }
    for path in refused {
        assert!(
            workspace.files(FilesAction::Read(p(path))).is_err(),
            "{path}"
        );
        assert!(workspace.write(path, "no").is_err(), "{path}");
        assert!(
            workspace.files(FilesAction::List(p(path))).is_err(),
            "{path}"
        );
        assert!(
            workspace.files(FilesAction::Delete(vec![p(path)])).is_err(),
            "{path}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(outside.path.join("secret")).unwrap(),
        "SECRET"
    );
    let FilesReply::Entries(entries) = workspace.files(FilesAction::List(p(""))).unwrap() else {
        panic!();
    };
    assert!(
        entries
            .iter()
            .any(|entry| entry.name == p("junction") && !entry.is_directory)
    );
    for name in [
        "note.txt:stream",
        "trailing.",
        "trailing ",
        "NUL",
        "nul.txt",
        "COM1.log",
        "CONIN$",
        "a<b",
        "a|b",
        "real/aux",
    ] {
        assert!(workspace.write(name, "no").is_err(), "{name}");
        assert!(
            workspace.files(FilesAction::Mkdir(p(name))).is_err(),
            "{name}"
        );
    }
    workspace.write("real/file", "x").unwrap();
    for name in ["bad:name", "trailing.", "PRN"] {
        assert!(
            workspace
                .files(FilesAction::Rename {
                    path: p("real/file"),
                    name: p(name)
                })
                .is_err(),
            "{name}"
        );
    }
    assert!(std::fs::read_dir(&workspace.path).unwrap().all(|entry| {
        let name = entry.unwrap().file_name();
        !name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("NUL")
    }));
}

#[cfg(windows)]
#[test]
fn windows_paths_travel_slash_separated_and_git_folders_ignore_case() {
    let workspace = Workspace::new();
    workspace.files(FilesAction::Mkdir(p("a"))).unwrap();
    assert_eq!(
        workspace.write("a\\b.txt", "x").unwrap(),
        FilesReply::Path(p("a/b.txt"))
    );
    std::fs::create_dir(workspace.path.join(".GIT")).unwrap();
    let FilesReply::Entries(entries) = workspace.files(FilesAction::List(p("a"))).unwrap() else {
        panic!();
    };
    assert_eq!(entries[0].path, p("a/b.txt"));
    let FilesReply::Entries(entries) = workspace.files(FilesAction::List(p(""))).unwrap() else {
        panic!();
    };
    assert!(!entries.iter().any(|entry| entry.name == p(".GIT")));
}

#[cfg(windows)]
#[test]
fn windows_delete_moves_to_the_recycle_bin_and_refuses_volumes_without_one() {
    let workspace = Workspace::new();
    workspace.write("recycled.txt", "x").unwrap();
    workspace
        .files(FilesAction::Delete(vec![p("recycled.txt")]))
        .unwrap();
    assert!(!workspace.path.join("recycled.txt").exists());
    let local = workspace.path.to_str().unwrap();
    let share = format!(r"\\localhost\{}${}", &local[..1], &local[2..]);
    if !Path::new(&share).is_dir() {
        return;
    }
    std::fs::write(workspace.path.join("kept.txt"), "x").unwrap();
    let root = root::Root::open(Path::new(&share)).unwrap();
    let source = root.mutation(Path::new("kept.txt")).unwrap();
    assert!(trash::move_to_trash(&root, &source).is_err());
    assert!(workspace.path.join("kept.txt").exists());
}

fn absolute(path: &Path) -> ServerPath {
    ServerPath::from_path(path).unwrap()
}

#[test]
fn entries_list_files_and_folders_anywhere_sorted_ignoring_case() {
    let workspace = Workspace::new();
    std::fs::create_dir(workspace.path.join("docs")).unwrap();
    std::fs::create_dir(workspace.path.join(".git")).unwrap();
    std::fs::write(workspace.path.join("Cargo.toml"), "").unwrap();
    std::fs::write(workspace.path.join("build.rs"), "").unwrap();
    let listed: Vec<_> = entries(&absolute(&workspace.path))
        .unwrap()
        .into_iter()
        .map(|entry| (String::from_utf8(entry.name.0).unwrap(), entry.is_directory))
        .collect();
    assert_eq!(
        listed,
        [
            (".git".to_owned(), true),
            ("build.rs".to_owned(), false),
            ("Cargo.toml".to_owned(), false),
            ("docs".to_owned(), true),
        ]
    );
    assert_eq!(
        folders(&absolute(&workspace.path)).unwrap(),
        [p(".git"), p("docs")]
    );
}

#[cfg(unix)]
#[test]
fn entries_count_links_to_folders_as_folders() {
    let workspace = Workspace::new();
    std::fs::create_dir(workspace.path.join("real")).unwrap();
    std::fs::write(workspace.path.join("file"), "").unwrap();
    symlink(
        workspace.path.join("real"),
        workspace.path.join("folder-link"),
    )
    .unwrap();
    symlink(
        workspace.path.join("file"),
        workspace.path.join("file-link"),
    )
    .unwrap();
    symlink(workspace.path.join("gone"), workspace.path.join("broken")).unwrap();
    let folders: Vec<_> = entries(&absolute(&workspace.path))
        .unwrap()
        .into_iter()
        .filter(|entry| entry.is_directory)
        .map(|entry| entry.name)
        .collect();
    assert_eq!(folders, [p("folder-link"), p("real")]);
}

#[test]
fn entries_refuse_relative_missing_and_file_paths() {
    let workspace = Workspace::new();
    std::fs::write(workspace.path.join("file"), "").unwrap();
    assert!(entries(&p("relative")).is_err());
    assert!(entries(&absolute(&workspace.path.join("missing"))).is_err());
    assert!(entries(&absolute(&workspace.path.join("file"))).is_err());
}
