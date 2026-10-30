use std::sync::{Arc, mpsc};

use super::*;
use crate::model::project_apps::Listed;
use crate::opener::ProjectApp;

fn listed(id: &str, name: &str) -> Listed {
    Listed {
        app: ProjectApp {
            id: id.into(),
            name: name.into(),
            path: PathBuf::from(format!("/Applications/{name}.app")),
        },
        icon: None,
    }
}

/// Finder and two editors, with launches reported instead of run.
fn install(model: &mut AppModel) -> mpsc::Receiver<(PathBuf, String)> {
    let (sender, launches) = mpsc::channel();
    model.project_apps.listed = Some(vec![
        listed("com.apple.finder", "Finder"),
        listed("dev.zed.Zed", "Zed"),
        listed("com.apple.dt.Xcode", "Xcode"),
    ]);
    model.project_apps.open = Arc::new(move |directory, app| {
        let _ = sender.send((directory.to_owned(), app.to_owned()));
        Ok(())
    });
    launches
}

fn launched(launches: &mpsc::Receiver<(PathBuf, String)>) -> (PathBuf, String) {
    launches
        .recv_timeout(Duration::from_secs(5))
        .expect("the project opened")
}

#[gpui::test]
fn the_control_opens_the_project_in_the_chosen_app_and_remembers_it(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.open_terminal_tab(project).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(600.0)));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("open-in-ide").is_none(),
        "hidden until the installed apps are known"
    );

    let launches = view.update(cx, |model, cx| {
        let launches = install(model);
        cx.notify();
        launches
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.default_project_app().map(|app| app.app.name.as_str()),
            Some("Zed"),
            "the most preferred editor until one is chosen"
        );
    });
    let primary = cx.debug_bounds("open-in-ide-primary").expect("open button");
    cx.simulate_click(primary.center(), Modifiers::default());
    assert_eq!(
        launched(&launches),
        (std::env::temp_dir(), "dev.zed.Zed".to_owned())
    );
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.settings.openers.project_target, None,
            "only choosing from the menu changes the saved app"
        );
    });

    let chevron = cx.debug_bounds("open-in-ide-menu").expect("menu button");
    cx.simulate_click(chevron.center(), Modifiers::default());
    cx.run_until_parked();
    assert!(cx.debug_bounds("menu-heading-Editors & IDEs").is_some());
    assert!(cx.debug_bounds("menu-icon-Xcode").is_some());
    let labels: Vec<_> = ["menu-label-Finder", "menu-label-Xcode", "menu-label-Zed"]
        .into_iter()
        .map(|label| cx.debug_bounds(label).expect("listed").top())
        .collect();
    assert!(
        labels.is_sorted(),
        "Finder, then editors by name: {labels:?}"
    );
    let xcode = cx.debug_bounds("menu-label-Xcode").expect("Xcode");
    cx.simulate_click(xcode.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        launched(&launches),
        (std::env::temp_dir(), "com.apple.dt.Xcode".to_owned())
    );
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(
            model.settings.openers.project_target.as_deref(),
            Some("com.apple.dt.Xcode")
        );
        assert_eq!(
            model.default_project_app().map(|app| app.app.name.as_str()),
            Some("Xcode")
        );
    });
}

#[gpui::test]
fn projects_on_other_computers_have_no_open_in_ide_control(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let server = ServerId::new();
    state
        .apply_catalog(server, &servers::page(2, ProjectId::new(), &[]))
        .expect("remote catalog");
    let project = state
        .add_project(server, PathBuf::from("/srv/app"))
        .expect("project");
    let (boot, _requests, _remotes) = remote_boot(state, vec![servers::entry(server, "box")]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let launches = view.update(cx, |model, cx| {
        let launches = install(model);
        model.select_project(project, cx);
        assert_eq!(model.project_apps_directory(), None);
        model.open_project_in(1, cx);
        launches
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("open-in-ide").is_none());
    assert!(launches.try_recv().is_err());
}
