use super::*;
use crate::views::{composer::ComposerEvent, native_modal::NativeModal};
use gpui::Focusable;
use muxy_app_core::modal::{ModalItem, ModalOptions};

fn composer_text(view: &Entity<AppModel>, cx: &VisualTestContext) -> String {
    view.read_with(cx, |model, cx| {
        model
            .composer
            .view
            .as_ref()
            .expect("composer")
            .read(cx)
            .draft
            .text
            .clone()
    })
}

#[gpui::test]
fn shared_panel_header_preserves_composer_actions(cx: &mut TestAppContext) {
    use muxy_app_core::settings::{ComposerPosition, ComposerPresentation};
    use muxy_ui::panel::PanelId;

    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.composer.presentation = ComposerPresentation::Panel;
    boot.settings.composer.position = ComposerPosition::Right;
    boot.settings.composer.pinned = true;
    boot.settings.composer.broadcast = false;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    cx.simulate_keystrokes("cmd-i");
    cx.simulate_input("keep editing");
    for selector in ["composer-position", "composer-pin", "composer-broadcast"] {
        cx.run_until_parked();
        let bounds = cx.debug_bounds(selector).expect("panel action");
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.run_until_parked();
    }
    view.read_with(cx, |model, cx| {
        assert_eq!(model.settings.composer.position, ComposerPosition::Bottom);
        assert!(!model.settings.composer.pinned);
        assert!(model.settings.composer.broadcast);
        let composer = model.composer.view.as_ref().expect("composer").read(cx);
        assert_eq!(
            model
                .panels
                .placement(&PanelId::new(crate::views::composer::PANEL)),
            Some(&composer.placement())
        );
        assert_eq!(composer.draft.text, "keep editing");
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(!model.settings.composer.broadcast));
    let bounds = cx
        .debug_bounds("composer-floating")
        .expect("floating action");
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.settings.composer.presentation,
            ComposerPresentation::Floating
        );
        assert!(
            model
                .panels
                .placement(&PanelId::new(crate::views::composer::PANEL))
                .is_none()
        );
    });
    cx.simulate_keystrokes("cmd-i");
    cx.simulate_keystrokes("cmd-i");
    view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |composer, cx| {
            composer.settings.presentation = ComposerPresentation::Panel;
            cx.emit(ComposerEvent::Preferences(composer.settings.clone()));
            cx.notify();
        });
    });
    cx.run_until_parked();
    let bounds = cx.debug_bounds("composer-close").expect("close action");
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.composer.view.is_none()));
}

#[gpui::test]
fn composer_shortcut_focus_drafts_and_panel_modes(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let project = state.add_project(std::env::temp_dir()).expect("project");
    state.select_project(home).expect("select home");
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    cx.simulate_keystrokes("cmd-i");
    cx.simulate_input("home draft");
    assert_eq!(composer_text(&view, cx), "home draft");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.composer.view.is_none()));
    cx.simulate_keystrokes("cmd-i");
    assert_eq!(composer_text(&view, cx), "home draft");
    view.update(cx, |model, cx| model.select_project(project, cx));
    assert_eq!(composer_text(&view, cx), "");
    cx.update(|window, cx| {
        view.read(cx)
            .composer
            .view
            .as_ref()
            .expect("composer")
            .focus_handle(cx)
            .focus(window);
    });
    cx.simulate_input("project draft");
    view.update(cx, |model, cx| model.select_project(home, cx));
    assert_eq!(composer_text(&view, cx), "home draft");
    for position in [
        muxy_app_core::settings::ComposerPosition::Right,
        muxy_app_core::settings::ComposerPosition::Bottom,
    ] {
        for pinned in [true, false] {
            view.update(cx, |model, cx| {
                let panel = model.composer.view.clone().expect("composer");
                panel.update(cx, |panel, cx| {
                    panel.settings.position = position;
                    panel.settings.pinned = pinned;
                    cx.emit(ComposerEvent::Preferences(panel.settings.clone()));
                    cx.notify();
                });
            });
            cx.run_until_parked();
            let bounds = cx.debug_bounds("composer-body").expect("panel body");
            assert!(bounds.size.width > px(200.0));
            assert!(bounds.size.height > px(80.0));
            assert!(bounds.bottom() <= px(600.0));
            assert!(bounds.right() <= px(1000.0));
            cx.simulate_click(bounds.center(), Modifiers::default());
            cx.run_until_parked();
            view.read_with(cx, |model, _| assert!(model.composer.view.is_some()));
        }
    }
}

fn attached_composer(
    cx: &mut TestAppContext,
) -> (
    Entity<AppModel>,
    &mut VisualTestContext,
    std::sync::mpsc::Receiver<(u64, Work)>,
) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let session = SessionId::new(12).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (mut boot, requests) = stub_boot(state);
    boot.settings.composer.clear_after_sending = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.receive(
            (
                1,
                Update::Attached {
                    pane,
                    session,
                    attachment: attachment(),
                    created: false,
                },
            ),
            cx,
        );
    });
    cx.simulate_keystrokes("cmd-i");
    (view, cx, requests)
}

fn delivery(
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
) -> (
    Vec<u8>,
    async_channel::Sender<std::result::Result<(), String>>,
) {
    requests
        .try_iter()
        .find_map(|(_, work)| match work {
            Work::WriteInput {
                bytes, completion, ..
            } => Some((bytes, completion)),
            _ => None,
        })
        .expect("acknowledged terminal input")
}

#[gpui::test]
fn composer_delivery_preserves_edits_and_failures_then_clears_confirmed_draft(
    cx: &mut TestAppContext,
) {
    let (view, cx, requests) = attached_composer(cx);
    cx.simulate_input("first");
    cx.simulate_keystrokes("cmd-enter");
    cx.run_until_parked();
    let (bytes, completion) = delivery(&requests);
    assert_eq!(bytes, b"\x15first\r");
    cx.simulate_input(" edited");
    completion.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "first edited");
    cx.simulate_keystrokes("cmd-shift-enter");
    cx.run_until_parked();
    let (bytes, completion) = delivery(&requests);
    assert_eq!(bytes, b"\x15first edited");
    completion
        .try_send(Err("writer failed".into()))
        .expect("failure");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "first edited");
    view.read_with(cx, |model, cx| {
        assert!(
            model
                .composer
                .view
                .as_ref()
                .expect("composer")
                .read(cx)
                .error
                .as_ref()
                .expect("error")
                .contains("Draft preserved")
        );
    });
    cx.simulate_keystrokes("cmd-enter");
    cx.run_until_parked();
    delivery(&requests).1.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "");
}

#[gpui::test]
fn native_modal_stream_selection_and_escape_complete_once(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for cancel in [false, true] {
        let result = cx.update(|window, cx| {
            let theme = view.read(cx).theme.clone();
            let metrics = view.read(cx).metrics;
            let mut result = None;
            let picker = cx.new(|cx| {
                let (mut picker, receiver, _) =
                    NativeModal::new(ModalOptions::default(), theme, metrics, cx);
                let token = picker.token();
                picker.feed(
                    token,
                    vec![
                        ModalItem::new("en", "English"),
                        ModalItem::new("fr", "French"),
                    ],
                    cx,
                );
                picker.finish(token, cx);
                result = Some(receiver);
                picker
            });
            picker.focus_handle(cx).focus(window);
            view.update(cx, |model, cx| {
                model.overlay = Some(Overlay::Native(picker));
                cx.notify();
            });
            result.expect("receiver")
        });
        cx.simulate_input("French");
        cx.simulate_keystrokes(if cancel { "escape" } else { "enter" });
        cx.run_until_parked();
        assert_eq!(
            result.try_recv().expect("completion").map(|item| item.id),
            (!cancel).then(|| "fr".into())
        );
        view.update(cx, AppModel::dismiss_overlay);
        cx.run_until_parked();
        assert!(result.try_recv().is_err());
    }
}

#[gpui::test]
fn composer_broadcast_deduplicates_sessions_and_retains_partial_failure(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    let third = state.split_pane(second, Direction::Right).expect("split");
    for (pane, id) in [(first, 31), (second, 31), (third, 32)] {
        state
            .set_pane_session(pane, SessionId::new(id))
            .expect("session");
    }
    let (mut boot, requests) = stub_boot(state);
    boot.settings.composer.broadcast = true;
    boot.settings.composer.clear_after_sending = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        for (index, pane) in [first, second, third].into_iter().enumerate() {
            let mut attachment = attachment();
            attachment.channel =
                muxy_protocol::ChannelId(u32::try_from(index + 1).expect("channel"));
            model.receive(
                (
                    1,
                    Update::Attached {
                        pane,
                        session: model.pane_session(pane).expect("session"),
                        attachment,
                        created: false,
                    },
                ),
                cx,
            );
        }
    });
    cx.simulate_keystrokes("cmd-i");
    cx.simulate_input("broadcast");
    cx.simulate_keystrokes("cmd-enter");
    cx.run_until_parked();
    delivery(&requests).1.try_send(Ok(())).expect("first ack");
    cx.run_until_parked();
    delivery(&requests)
        .1
        .try_send(Err("second writer failed".into()))
        .expect("second failure");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "broadcast");
    view.read_with(cx, |model, cx| {
        let panel = model.composer.view.as_ref().expect("composer").read(cx);
        assert!(
            panel
                .error
                .as_ref()
                .expect("partial result")
                .starts_with("1/2 terminals sent.")
        );
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::WriteInput { .. }))
    );
}

#[gpui::test]
fn floating_composer_closes_after_preflight_and_preserves_failed_draft(cx: &mut TestAppContext) {
    let (view, cx, requests) = attached_composer(cx);
    view.update(cx, |model, cx| {
        let panel = model.composer.view.clone().unwrap();
        panel.update(cx, |panel, cx| {
            panel.settings.presentation = muxy_app_core::settings::ComposerPresentation::Floating;
            panel.settings.clear_on_close = true;
            cx.emit(ComposerEvent::Preferences(panel.settings.clone()));
        });
    });
    cx.simulate_input("preserve failed send");
    cx.simulate_keystrokes("cmd-enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.composer.view.is_none()));
    let (bytes, completion) = delivery(&requests);
    assert_eq!(bytes, b"\x15preserve failed send\r");
    completion.try_send(Err("disconnected".into())).unwrap();
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-i");
    assert_eq!(composer_text(&view, cx), "preserve failed send");
    let entering = cx.debug_bounds("floating-composer-card").unwrap();
    assert!(entering.size.width < px(570.0));
    cx.executor()
        .advance_clock(crate::views::composer::TRANSITION);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bounds = cx.debug_bounds("floating-composer-card").unwrap();
    assert_eq!(bounds.size, size(px(570.0), px(236.0)));
    assert_eq!(bounds.center(), gpui::point(px(500.0), px(300.0)));
    let expand = cx.debug_bounds("composer-expand").unwrap();
    cx.simulate_click(expand.center(), Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.settings.composer.expanded));
    cx.simulate_keystrokes("cmd-+");
    view.read_with(cx, |model, cx| {
        assert_eq!(
            model
                .composer
                .view
                .as_ref()
                .unwrap()
                .read(cx)
                .settings
                .font_size
                .to_bits(),
            14.0_f32.to_bits()
        );
    });
}
