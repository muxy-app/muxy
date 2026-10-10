use super::*;
use muxy_core::shortcuts::ShortcutId;

fn shown(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |model, _| {
        model
            .shortcut_hints
            .label(ShortcutId::SelectTab1, &model.settings.keymap)
            .is_some()
    })
}

fn hold(view: &Entity<AppModel>, held: Modifiers, cx: &mut VisualTestContext) {
    view.update(cx, |model, _| model.shortcut_hints.held = held);
}

#[gpui::test]
fn hints_follow_the_keyboard_when_a_release_is_never_reported(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let command = Modifiers::command();

    hold(&view, command, cx);
    view.update(cx, |model, cx| model.update_shortcut_hints(command, cx));
    cx.executor().advance_clock(Duration::from_millis(499));
    assert!(!shown(&view, cx));
    cx.executor().advance_clock(Duration::from_millis(1));
    assert!(shown(&view, cx));
    cx.executor().advance_clock(Duration::from_secs(2));
    assert!(shown(&view, cx));

    hold(&view, Modifiers::default(), cx);
    cx.executor().advance_clock(Duration::from_millis(100));
    assert!(!shown(&view, cx));

    hold(&view, command, cx);
    view.update(cx, |model, cx| model.update_shortcut_hints(command, cx));
    cx.executor().advance_clock(Duration::from_millis(500));
    assert!(shown(&view, cx));
}

#[gpui::test]
fn hints_stay_hidden_when_the_release_is_lost_before_they_appear(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let command = Modifiers::command();

    hold(&view, command, cx);
    view.update(cx, |model, cx| model.update_shortcut_hints(command, cx));
    cx.executor().advance_clock(Duration::from_millis(200));
    hold(&view, Modifiers::default(), cx);
    cx.executor().advance_clock(Duration::from_millis(300));
    assert!(!shown(&view, cx));
    cx.executor().advance_clock(Duration::from_secs(2));
    assert!(!shown(&view, cx));
}

#[gpui::test]
fn a_reported_release_hides_hints_at_once_and_they_stay_hidden(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let command = Modifiers::command();

    hold(&view, command, cx);
    view.update(cx, |model, cx| model.update_shortcut_hints(command, cx));
    cx.executor().advance_clock(Duration::from_millis(500));
    assert!(shown(&view, cx));

    view.update(cx, |model, cx| {
        model.update_shortcut_hints(Modifiers::default(), cx);
    });
    assert!(!shown(&view, cx));
    cx.executor().advance_clock(Duration::from_secs(2));
    assert!(!shown(&view, cx));
}
