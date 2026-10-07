use super::*;
use muxy_app_core::settings::CustomCommand;

fn command() -> CustomCommand {
    CustomCommand {
        id: "tests".into(),
        name: "Run project tests".into(),
        command: "printf 'hello world' && echo done".into(),
    }
}

fn configured_boot(state: AppState) -> (Boot, std::sync::mpsc::Receiver<(u64, Work)>) {
    let (mut boot, requests) = stub_boot(state);
    boot.settings.commands.push(command());
    boot.settings.keymap = boot
        .settings
        .keymap
        .with_binding("command.tests", Some("cmd-alt-y".parse().unwrap()))
        .unwrap();
    (boot, requests)
}

#[gpui::test]
fn custom_command_shortcut_creates_named_tab_and_submits_once_after_attach(
    cx: &mut TestAppContext,
) {
    let project_dir = tempfile::tempdir().unwrap();
    let mut state = AppState::bootstrap().unwrap();
    let project = state
        .add_project(ServerId::local(), project_dir.path().to_owned())
        .unwrap();
    state.select_project(project).unwrap();
    let (boot, requests) = configured_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    requests.try_iter().for_each(drop);
    cx.simulate_keystrokes("cmd-alt-y");
    cx.run_until_parked();
    let pane = view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        let tab = &model.state.current_project().tabs[0];
        assert_eq!(tab.title(None), "Run project tests");
        let pane = model.active_pane().unwrap();
        assert_eq!(
            model.state.startup_command(pane),
            Some(command().command.as_str())
        );
        pane
    });
    let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
    assert!(!work.iter().any(|work| matches!(work, Work::Input(..))));
    assert!(work.iter().any(|work| matches!(work, Work::Attach { pane: target, directory, .. } if *target == pane && directory == project_dir.path())));
    let session = SessionId::new(7).unwrap();
    view.update(cx, |model, cx| {
        model.receive_attached(ServerId::local(), pane, session, attachment(), true, cx);
        model.receive_attached(ServerId::local(), pane, session, attachment(), false, cx);
    });
    let typed: Vec<_> = requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::Input(_, bytes) => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(typed, [format!("{}\r", command().command).into_bytes()]);
    view.read_with(cx, |model, _| {
        assert!(model.state.startup_command(pane).is_none());
        let saved = store::load(&model.path).unwrap();
        assert!(saved.startup_command(pane).is_none());
    });
}
