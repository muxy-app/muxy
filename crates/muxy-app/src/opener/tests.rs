#[cfg(not(target_os = "macos"))]
use std::os::unix::ffi::OsStringExt;

use super::*;

#[cfg(not(target_os = "macos"))]
struct Temp(PathBuf);

#[cfg(not(target_os = "macos"))]
impl Temp {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("muxy-openers-{}", muxy_app_core::PaneId::new()));
        std::fs::create_dir(&root).expect("temp directory");
        Self(root)
    }
}

#[cfg(not(target_os = "macos"))]
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(target_os = "macos")]
fn context() -> OpenContext {
    OpenContext {
        project: muxy_app_core::ProjectId::new(),
        pane: muxy_app_core::PaneId::new(),
        server: muxy_app_core::ServerId::local(),
        directory: "/project/sub directory".into(),
        project_directory: "/project".into(),
    }
}

fn file(path: impl Into<PathBuf>) -> FileLocation {
    FileLocation {
        path: path.into(),
        line: Some(12),
        column: Some(3),
    }
}

#[cfg(target_os = "macos")]
#[test]
fn editor_selection_and_failed_launch_fall_back_without_losing_context() {
    let editors = [
        Editor {
            bundle: "first".into(),
            name: "First".into(),
            path: "/Applications/First.app".into(),
            rank: 0,
        },
        Editor {
            bundle: "preferred".into(),
            name: "Preferred".into(),
            path: "/Applications/Preferred.app".into(),
            rank: 1,
        },
    ];
    let request = OpenRequest {
        target: Target::File(file("/tmp/file.rs")),
        context: context(),
    };
    for (preference, application) in [
        ("preferred", "/Applications/Preferred.app"),
        ("missing", "/Applications/First.app"),
    ] {
        let settings = muxy_app_core::settings::OpenerSettings {
            project_target: Some(preference.into()),
            ..muxy_app_core::settings::OpenerSettings::default()
        };
        let mut calls = 0;
        open_with(
            &request,
            &settings,
            || &editors,
            |command| {
                calls += 1;
                if calls == 1 {
                    assert_eq!(
                        command,
                        &Launch::system([
                            "-a".into(),
                            application.into(),
                            "/project".into(),
                            "/tmp/file.rs".into()
                        ])
                    );
                    Err(io::Error::other("editor failed"))
                } else {
                    assert_eq!(command, &finder(Path::new("/tmp/file.rs")));
                    Ok(())
                }
            },
        )
        .expect("Finder fallback");
        assert_eq!(calls, 2);
        assert_eq!(settings.project_target.as_deref(), Some(preference));
    }
    let mut remote = request;
    remote.context.server = muxy_app_core::ServerId::new();
    assert!(
        open_with(
            &remote,
            &muxy_app_core::settings::OpenerSettings::default(),
            || &editors,
            |_| panic!("remote file must not launch")
        )
        .is_err()
    );
    assert!(resolve("/etc/hosts", &remote.context).is_none());
    assert!(resolve("https://example.com", &remote.context).is_some());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn linux_openers_preserve_literal_paths_and_use_the_file_manager_for_reveal() {
    let temp = Temp::new();
    let directory = temp.0.join("project space");
    std::fs::create_dir(&directory).expect("project");
    let location = file(PathBuf::from(OsString::from_vec(
        b"/tmp/$(touch injected) '\xff.rs".to_vec(),
    )));
    let editor = Editor {
        bundle: "com.microsoft.VSCode".into(),
        name: "Visual Studio Code".into(),
        path: "/usr/bin/code".into(),
        rank: 0,
    };
    let command = editor.command(&location, &directory);
    assert_eq!(command.program, Path::new("/usr/bin/code"));
    assert_eq!(
        command.args,
        [
            directory.as_os_str().to_owned(),
            "--goto".into(),
            OsString::from_vec(b"/tmp/$(touch injected) '\xff.rs:12:3".to_vec())
        ]
    );
    assert_eq!(
        Launch::system([location.path.as_os_str().to_owned()]).program,
        Path::new("xdg-open")
    );
    let reveal = finder(&location.path);
    assert_eq!(reveal.select.as_deref(), Some(location.path.as_path()));
    assert_eq!(
        (reveal.program.as_path(), reveal.args.as_slice()),
        (Path::new("xdg-open"), [OsString::from("/tmp")].as_slice()),
        "without a file manager that selects, the folder opens"
    );
    let folder = finder(&directory);
    assert_eq!(folder.select, None);
    assert_eq!(folder.args, [directory.into_os_string()]);
}

fn project_app(id: &str) -> ProjectApp {
    ProjectApp {
        id: id.into(),
        name: id.into(),
        path: PathBuf::from(format!("/Applications/{id}.app")),
    }
}

#[test]
fn the_project_button_prefers_the_chosen_app_then_an_editor_then_the_file_manager() {
    let file_manager = project_app(FINDER);
    let apps = [
        file_manager.clone(),
        project_app("zed"),
        project_app("code"),
    ];
    let chosen = |chosen| default_project_app(&apps, chosen).map(|app| app.id.as_str());
    assert_eq!(chosen(Some("code")), Some("code"));
    assert_eq!(chosen(Some(FINDER)), Some(FINDER));
    assert_eq!(chosen(Some("uninstalled")), Some("zed"));
    assert_eq!(chosen(None), Some("zed"));
    assert_eq!(
        default_project_app(std::slice::from_ref(&file_manager), Some("code")),
        Some(&file_manager)
    );
    assert_eq!(default_project_app::<ProjectApp>(&[], None), None);
}

#[test]
fn projects_open_as_folders_in_the_chosen_app() {
    let directory = std::env::temp_dir();
    let editors = [Editor {
        bundle: "editor".into(),
        name: "Editor".into(),
        path: "/Applications/Editor.app".into(),
        rank: 0,
    }];
    let mut launches = 0;
    open_project_with(&directory, FINDER, &editors, |command| {
        launches += 1;
        assert_eq!(command, &finder(&directory));
        Ok(())
    })
    .expect("file manager");
    open_project_with(&directory, "editor", &editors, |command| {
        launches += 1;
        #[cfg(target_os = "macos")]
        let expected = Launch::system([
            "-a".into(),
            "/Applications/Editor.app".into(),
            directory.clone().into_os_string(),
        ]);
        #[cfg(not(target_os = "macos"))]
        let expected = Launch::program("/Applications/Editor.app", [directory.clone().into()]);
        assert_eq!(command, &expected);
        Ok(())
    })
    .expect("editor");
    assert_eq!(launches, 2);
    assert!(
        open_project_with(&directory, "uninstalled", &editors, |_| {
            panic!("an uninstalled app must not launch")
        })
        .is_err()
    );
}

#[test]
fn the_file_manager_comes_before_editors_in_their_preferred_order() {
    let editor = |bundle: &str, name: &str, rank| Editor {
        bundle: bundle.into(),
        name: name.into(),
        path: PathBuf::from(format!("/Applications/{name}.app")),
        rank,
    };
    let apps = project_apps_from(&[editor("zed", "Zed", 0), editor("code", "Code", 1)]);
    let listed: Vec<_> = apps
        .iter()
        .map(|app| (app.id.as_str(), app.is_file_manager()))
        .collect();
    assert_eq!(listed, [(FINDER, true), ("zed", false), ("code", false)]);
    assert_eq!(apps[1].name, "Zed");
    assert_eq!(apps[1].path, Path::new("/Applications/Zed.app"));
}
