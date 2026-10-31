use super::*;
use crate::views::overlays::Overlay;
use muxy_app_core::settings::ComposerPresentation;

fn attached_terminal(
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext, PaneId) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let session = SessionId::new(12).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.receive(
            (
                ServerId::local(),
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
    cx.run_until_parked();
    (view, cx, pane)
}

fn terminal_scroll_shown(model: &mut AppModel, pane: PaneId, cx: &mut Context<AppModel>) -> bool {
    model.sync_pane_focus(cx);
    model.grids[&pane].view.read(cx).native_visible
}

#[gpui::test]
fn every_layer_hides_terminal_scroll_and_blocks_pane_drag(cx: &mut TestAppContext) {
    let (view, cx, pane) = attached_terminal(cx);
    view.update(cx, |model, cx| {
        assert!(terminal_scroll_shown(model, pane, cx));
        assert!(!model.coverage().blocks_pane_drag());

        model.overlay = Some(Overlay::Updates);
        assert!(!terminal_scroll_shown(model, pane, cx));
        let coverage = model.coverage();
        assert!(coverage.hides_webviews() && !coverage.hides_spinners());
        assert!(coverage.blocks_pane_drag());

        model.overlay = Some(Overlay::Webview);
        assert!(model.coverage().hides_spinners());

        model.overlay = None;
        model.close_prompt = Some(Task::ready(()));
        assert!(!terminal_scroll_shown(model, pane, cx));
        let coverage = model.coverage();
        assert!(coverage.hides_webviews() && coverage.hides_overlay_webviews());
        assert!(!coverage.hides_spinners());
        assert!(coverage.blocks_pane_drag());

        model.close_prompt = None;
        assert!(terminal_scroll_shown(model, pane, cx));
    });
}

#[gpui::test]
fn the_floating_composer_covers_the_workspace(cx: &mut TestAppContext) {
    let (view, cx, pane) = attached_terminal(cx);
    view.update_in(cx, |model, window, cx| {
        model.settings.composer.presentation = ComposerPresentation::Floating;
        model.toggle_composer(window, cx);
        assert!(model.composer.view.is_some());
        assert!(!terminal_scroll_shown(model, pane, cx));
        let coverage = model.coverage();
        assert!(coverage.hides_webviews() && coverage.hides_spinners());
        assert!(coverage.blocks_pane_drag());
    });
}

#[gpui::test]
fn a_panel_composer_covers_only_its_rectangle(cx: &mut TestAppContext) {
    let (view, cx, pane) = attached_terminal(cx);
    view.update_in(cx, |model, window, cx| {
        model.settings.composer.presentation = ComposerPresentation::Panel;
        model.settings.composer.pinned = false;
        model.toggle_composer(window, cx);
        assert!(model.composer.view.is_some());
        assert!(!terminal_scroll_shown(model, pane, cx));
        let coverage = model.coverage();
        assert!(!coverage.hides_webviews() && !coverage.hides_spinners());
        assert!(!coverage.blocks_pane_drag());

        model.settings.composer.pinned = true;
        assert!(terminal_scroll_shown(model, pane, cx));
    });
}
