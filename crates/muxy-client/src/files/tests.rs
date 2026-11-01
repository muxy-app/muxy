use super::*;
use crate::fake::{Fake, wait};

fn path(text: &str) -> ServerPath {
    ServerPath(text.as_bytes().to_vec())
}

/// A reply of another kind than `reply`.
fn other_than(reply: &FilesReply) -> FilesReply {
    if *reply == FilesReply::Done {
        FilesReply::Paths(Vec::new())
    } else {
        FilesReply::Done
    }
}

/// Runs a typed method and its async twin against `$reply`, then checks
/// that both refuse a reply of another kind.
macro_rules! case {
    ($fake:expr, $project:expr, $name:ident, $name_async:ident($($arg:expr),*),
        $action:expr, $reply:expr, $expected:expr) => {{
        let (fake, project): (&Fake, ProjectId) = ($fake, $project);
        let sent = RequestBody::Files(FilesRequest { project, action: $action });
        let (request, result) =
            fake.answer(ReplyBody::Files($reply), |client| client.$name(project, $($arg),*));
        assert_eq!(request, sent, stringify!($name));
        assert_eq!(result.expect(stringify!($name)), $expected, stringify!($name));
        let (request, result) = fake.answer(ReplyBody::Files($reply), |client| {
            wait(client.$name_async(project, $($arg),*))
        });
        assert_eq!(request, sent, stringify!($name_async));
        assert_eq!(result.expect(stringify!($name_async)), $expected, stringify!($name_async));
        let (_, result) = fake.answer(ReplyBody::Files(other_than(&$reply)), |client| {
            client.$name(project, $($arg),*)
        });
        assert!(
            matches!(result, Err(ClientError::UnexpectedReply(_))),
            "{} accepted another reply",
            stringify!($name)
        );
        let (_, result) = fake.answer(ReplyBody::Detached, |client| {
            wait(client.$name_async(project, $($arg),*))
        });
        assert!(
            matches!(result, Err(ClientError::UnexpectedReply(_))),
            "{} accepted another reply",
            stringify!($name_async)
        );
    }};
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One case per file action, read as a table"
)]
fn each_file_method_sends_its_action_and_accepts_only_its_reply() {
    let fake = Fake::start();
    let fake = &fake;
    let project = ProjectId::new();
    let entry = FileEntry {
        name: path("a"),
        path: path("docs/a"),
        is_directory: false,
        is_ignored: false,
    };
    case!(
        fake,
        project,
        list_directory,
        list_directory_async(path("docs")),
        FilesAction::List(path("docs")),
        FilesReply::Entries(vec![entry.clone()]),
        vec![entry.clone()]
    );
    let content = FileContent {
        path: path("a"),
        content: "text".into(),
        size: 4,
    };
    case!(
        fake,
        project,
        read_file,
        read_file_async(path("a")),
        FilesAction::Read(path("a")),
        FilesReply::Content(content.clone()),
        content.clone()
    );
    let bytes = FileBytes {
        path: path("a"),
        bytes: vec![0, 1],
    };
    case!(
        fake,
        project,
        read_file_bytes,
        read_file_bytes_async(path("a")),
        FilesAction::ReadBytes(path("a")),
        FilesReply::Bytes(bytes.clone()),
        bytes.clone()
    );
    let info = FileInfo {
        name: path("a"),
        path: path("a"),
        is_directory: true,
        size: 0,
    };
    case!(
        fake,
        project,
        file_info,
        file_info_async(path("a")),
        FilesAction::Stat(path("a")),
        FilesReply::Info(info.clone()),
        info.clone()
    );
    case!(
        fake,
        project,
        write_file,
        write_file_async(path("a"), "text".into()),
        FilesAction::Write {
            path: path("a"),
            content: "text".into()
        },
        FilesReply::Path(path("a")),
        path("a")
    );
    case!(
        fake,
        project,
        write_file_bytes,
        write_file_bytes_async(path("a"), vec![1]),
        FilesAction::WriteBytes {
            path: path("a"),
            bytes: vec![1]
        },
        FilesReply::Path(path("a")),
        path("a")
    );
    case!(
        fake,
        project,
        create_directory,
        create_directory_async(path("d")),
        FilesAction::Mkdir(path("d")),
        FilesReply::Path(path("d 2")),
        path("d 2")
    );
    case!(
        fake,
        project,
        rename_file,
        rename_file_async(path("a"), path("b")),
        FilesAction::Rename {
            path: path("a"),
            name: path("b")
        },
        FilesReply::Path(path("b")),
        path("b")
    );
    case!(
        fake,
        project,
        move_files,
        move_files_async(vec![path("a")], path("d")),
        FilesAction::Move {
            paths: vec![path("a")],
            into: path("d")
        },
        FilesReply::Paths(vec![path("d/a")]),
        vec![path("d/a")]
    );
    case!(
        fake,
        project,
        delete_files,
        delete_files_async(vec![path("a")]),
        FilesAction::Delete(vec![path("a")]),
        FilesReply::Done,
        ()
    );
    case!(
        fake,
        project,
        watch_files,
        watch_files_async(),
        FilesAction::Watch,
        FilesReply::Done,
        ()
    );
    case!(
        fake,
        project,
        unwatch_files,
        unwatch_files_async(),
        FilesAction::Unwatch,
        FilesReply::Done,
        ()
    );
}
