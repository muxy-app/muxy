use super::*;
use crate::picker::path_service::TypedPathState;
use std::sync::{Arc, Mutex};

fn show(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    server: ServerId,
    folders: RemoteFolders,
) -> Entity<ProjectPicker> {
    let picker = view.update(cx, |model, cx| {
        let picker = cx.new(|cx| {
            ProjectPicker::remote(folders, vec![], model.theme.clone(), model.metrics, cx)
        });
        model.show_project_picker(server, picker.clone(), cx);
        picker
    });
    settle(cx);
    picker
}

#[gpui::test]
fn command_enter_creates_a_nested_remote_folder_only_after_confirmation(cx: &mut TestAppContext) {
    let (view, cx, remote, _remotes) = connected_remote(cx);
    let before = view.read_with(cx, |model, _| model.state.projects().len());
    let created = Arc::new(Mutex::new(Vec::new()));
    let made = created.clone();
    let folders = RemoteFolders::new(
        "box".into(),
        "/home/dev/Home",
        |_| Ok(Vec::new()),
        |path, job| {
            assert_eq!(path, "/home/dev/Home/new/nested project");
            assert!(job > 0);
            Ok(TypedPathState::Missing)
        },
        move |path, _| {
            made.lock().expect("created").push(path.to_owned());
            Ok(())
        },
    );
    show(&view, cx, remote, folders);
    cx.simulate_input("new/nested project");
    settle(cx);
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    assert!(cx.has_pending_prompt());
    assert!(created.lock().expect("created").is_empty());
    cx.simulate_prompt_answer("Cancel");
    settle(cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.projects().len(), before);
    });
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    cx.simulate_prompt_answer("Create & Add");
    settle(cx);
    assert_eq!(
        *created.lock().expect("created"),
        ["/home/dev/Home/new/nested project"]
    );
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.projects().len(), before + 1);
        let project = model.state.current_project();
        assert_eq!(project.server_id, remote);
        assert_eq!(
            project.directory,
            PathBuf::from("/home/dev/Home/new/nested project")
        );
    });
}

#[gpui::test]
fn remote_creation_failure_keeps_the_form_and_does_not_add_a_project(cx: &mut TestAppContext) {
    let (view, cx, remote, _remotes) = connected_remote(cx);
    let before = view.read_with(cx, |model, _| model.state.projects().len());
    let folders = RemoteFolders::new(
        "box".into(),
        "/home/dev/Home",
        |_| Ok(Vec::new()),
        |_, _| Ok(TypedPathState::Missing),
        |_, _| Err("Permission denied".into()),
    );
    show(&view, cx, remote, folders);
    cx.simulate_input("new");
    settle(cx);
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    cx.simulate_prompt_answer("Create & Add");
    settle(cx);
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
        assert_eq!(model.state.projects().len(), before);
        assert!(model.close_prompt.is_none());
    });
}

#[gpui::test]
fn changing_or_dismissing_the_picker_revokes_remote_creation_confirmation(cx: &mut TestAppContext) {
    let (view, cx, remote, _remotes) = connected_remote(cx);
    let created = Arc::new(Mutex::new(Vec::new()));
    for dismiss in [false, true] {
        let made = created.clone();
        let folders = RemoteFolders::new(
            "box".into(),
            "/home/dev/Home",
            |_| Ok(Vec::new()),
            |_, _| Ok(TypedPathState::Missing),
            move |path, _| {
                made.lock().expect("created").push(path.to_owned());
                Ok(())
            },
        );
        let picker = show(&view, cx, remote, folders);
        cx.simulate_input("new");
        settle(cx);
        cx.simulate_keystrokes("cmd-enter");
        settle(cx);
        assert!(cx.has_pending_prompt());
        if dismiss {
            picker.update(cx, |_, cx| {
                cx.emit(crate::views::project_picker::PickerEvent::Dismiss);
            });
        } else {
            cx.simulate_input("-changed");
        }
        settle(cx);
        cx.simulate_prompt_answer("Create & Add");
        settle(cx);
        assert!(created.lock().expect("created").is_empty());
    }
}

#[gpui::test]
fn remote_files_and_check_errors_are_not_added_as_projects(cx: &mut TestAppContext) {
    let (view, cx, remote, _remotes) = connected_remote(cx);
    let before = view.read_with(cx, |model, _| model.state.projects().len());
    for outcome in [Ok(TypedPathState::NotDirectory), Err("Disconnected".into())] {
        let folders = RemoteFolders::new(
            "box".into(),
            "/home/dev/Home",
            |_| Ok(Vec::new()),
            move |_, _| outcome.clone(),
            |_, _| Err("This must not create anything".into()),
        );
        show(&view, cx, remote, folders);
        cx.simulate_input("file");
        settle(cx);
        cx.simulate_keystrokes("cmd-enter");
        settle(cx);
        assert!(!cx.has_pending_prompt());
        view.read_with(cx, |model, _| {
            assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
            assert_eq!(model.state.projects().len(), before);
        });
    }
}

pub(super) fn check_folder_operations(
    folders: &RemoteFolders,
    directory: &std::path::Path,
) -> Result {
    let nested = directory.join("new 'quoted' $(literal)/nested project");
    let nested_path = nested.to_string_lossy();
    assert_eq!(folders.check(&nested_path, 91)?, TypedPathState::Missing);
    folders.create(&nested_path, 92)?;
    assert!(nested.is_dir());
    assert_eq!(folders.check(&nested_path, 93)?, TypedPathState::Directory);
    folders.create(&nested_path, 94)?;
    let file = directory.join("ordinary file");
    std::fs::write(&file, "keep")?;
    assert_eq!(
        folders.check(&file.to_string_lossy(), 95)?,
        TypedPathState::NotDirectory
    );
    assert!(folders.create(&file.to_string_lossy(), 96).is_err());
    assert_eq!(std::fs::read_to_string(&file)?, "keep");
    assert!(folders.create("relative/path", 97).is_err());
    Ok(())
}
