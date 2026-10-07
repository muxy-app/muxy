use std::path::Path;

use muxy_app_core::{
    PaneId, ProjectId, ServerId,
    opener::{FileLocation, OpenContext, OpenRequest, Target},
};

fn request() -> OpenRequest {
    OpenRequest {
        target: Target::File(FileLocation {
            path: "/project/src/main.rs".into(),
            line: Some(12),
            column: Some(3),
        }),
        context: OpenContext {
            project: ProjectId::new(),
            pane: PaneId::new(),
            server: ServerId::local(),
            directory: "/project".into(),
            project_directory: "/project".into(),
        },
    }
}

#[test]
fn web_links_only_accept_explicit_supported_schemes_and_nonempty_hosts() {
    for value in [
        "http://localhost:3000/a?b=1#here",
        "https://example.com/a_(b)",
        "HTTPS://example.com/界",
    ] {
        assert_eq!(Target::web_url(value), Some(Target::Url(value.into())));
    }
    for value in [
        "javascript:alert(1)",
        "data:text/html,hello",
        "file:///etc/hosts",
        "https:///missing",
        "https://",
        "https://host/\nnext",
        "https://host/space here",
        "https://host\\@other",
    ] {
        assert!(Target::web_url(value).is_none(), "{value}");
    }
}

#[test]
fn local_targets_preserve_locations_and_decode_only_local_file_uris() {
    let existing = [
        "/project/src/main.rs",
        "/home/me/a b.rs",
        "/project/name:12",
        "/project/Makefile",
    ];
    let resolve = |text| {
        Target::file(
            text,
            Path::new("/project"),
            Some(Path::new("/home/me")),
            |path| existing.iter().any(|p| path == Path::new(p)),
        )
    };
    for text in ["src/main.rs:12:3", "/project/src/main.rs:12:3"] {
        assert_eq!(resolve(text), Some(request().target));
    }
    assert!(matches!(
        resolve("name:12"),
        Some(Target::File(FileLocation { line: None, .. }))
    ));
    assert!(matches!(resolve("Makefile"), Some(Target::File(_))));
    for text in [
        "file:///home/me/a%20b.rs",
        "file://localhost/home/me/a%20b.rs",
        "~/a b.rs",
    ] {
        assert_eq!(
            resolve(text),
            Some(Target::File(FileLocation {
                path: "/home/me/a b.rs".into(),
                line: None,
                column: None
            }))
        );
    }
    for text in [
        "file://remote/home/me/a%20b.rs",
        "file:///project/src/main.rs%00",
        "file:///project/src/main.rs%0a",
        "file:///project/src/main.rs?query",
        "file:///bad%GG",
        "https://host/file",
        "src/main.rs:0",
        "src/main.rs:4294967296",
        "missing",
    ] {
        assert!(resolve(text).is_none(), "{text}");
    }
}
