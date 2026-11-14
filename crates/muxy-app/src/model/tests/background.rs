use super::*;

#[gpui::test]
fn launch_explains_terminals_macos_ended_after_quit(cx: &mut TestAppContext) {
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.stopped_in_background = true;
    let (_view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let (title, _) = cx.pending_prompt().expect("background activity prompt");
    assert_eq!(title, crate::views::confirm::BACKGROUND_TITLE);
    cx.simulate_prompt_answer("Not Now");
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
}

#[gpui::test]
fn launch_stays_quiet_when_terminals_kept_running(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (_view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
}
