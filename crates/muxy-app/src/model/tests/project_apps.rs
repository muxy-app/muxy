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

#[gpui::test]
fn a_choice_that_cannot_be_saved_still_opens_and_says_so(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.open_terminal_tab(project).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    let launches = view.update(cx, |model, cx| {
        let settings = model.configuration_path("settings.toml");
        let _ = std::fs::remove_file(&settings);
        std::fs::create_dir_all(&settings).expect("block settings.toml");
        let launches = install(model);
        model.open_project_in(2, cx);
        launches
    });
    assert_eq!(
        launched(&launches),
        (std::env::temp_dir(), "com.apple.dt.Xcode".to_owned())
    );
    view.read_with(cx, |model, _| {
        assert_eq!(model.settings.openers.project_target, None);
        assert!(
            model
                .error
                .as_deref()
                .is_some_and(|error| error.starts_with("Could not save the default app")),
            "{:?}",
            model.error
        );
    });
}

#[gpui::test]
fn a_failed_launch_forgets_editors_that_were_uninstalled(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.open_terminal_tab(project).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    let installed = std::env::temp_dir();
    let uninstalled = installed.join(format!("muxy-uninstalled-{}.app", ProjectId::new()));
    view.update(cx, |model, cx| {
        let editor = |id: &str, path: &std::path::Path| Listed {
            app: ProjectApp {
                id: id.into(),
                name: id.into(),
                path: path.to_owned(),
            },
            icon: None,
        };
        model.project_apps.listed = Some(vec![
            listed("com.apple.finder", "Finder"),
            editor("gone", &uninstalled),
            editor("kept", &installed),
        ]);
        model.project_apps.open = Arc::new(|_, _| Err(io::Error::other("not installed")));
        model.open_project_in_default(cx);
    });
    cx.run_until_parked();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while view.read_with(cx, |model, _| model.error.is_none()) {
        assert!(std::time::Instant::now() < deadline, "the launch failed");
        cx.run_until_parked();
        std::thread::yield_now();
    }
    view.read_with(cx, |model, _| {
        let ids: Vec<_> = model
            .project_apps
            .listed
            .iter()
            .flatten()
            .map(|listed| listed.app.id.as_str())
            .collect();
        assert_eq!(ids, ["com.apple.finder", "kept"]);
        assert_eq!(
            model
                .default_project_app()
                .map(|listed| listed.app.id.as_str()),
            Some("kept")
        );
        assert!(
            model
                .error
                .as_deref()
                .is_some_and(|error| error.starts_with("Could not open project"))
        );
    });
}

/// The Open in IDE control in a window `width` wide whose sidebar is as wide
/// as it goes, with the tab strip beside it.
fn cramped(
    width: f32,
    cx: &mut TestAppContext,
) -> (gpui::Bounds<gpui::Pixels>, gpui::Bounds<gpui::Pixels>) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.open_terminal_tab(project).expect("tab");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    boot.settings.appearance.sidebar_expanded_width = Some(480.0);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(width), px(600.0)));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
        let _ = install(model);
        cx.notify();
    });
    cx.run_until_parked();
    (
        cx.debug_bounds("open-in-ide").expect("control"),
        cx.debug_bounds("tabs-scroll").expect("tab strip"),
    )
}

#[gpui::test]
fn the_control_drops_its_name_before_the_tab_strip_runs_out_of_room(cx: &mut TestAppContext) {
    let (named, _) = cramped(1400.0, cx);
    let (compact, tabs) = cramped(700.0, cx);
    assert!(
        compact.size.width < named.size.width,
        "{compact:?} {named:?}"
    );
    assert!(tabs.size.width > px(0.0), "{tabs:?}");
}
