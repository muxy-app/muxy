use super::*;
use crate::views::menu::{Command, Item};
use crate::views::project_editor::Field;
use crate::views::{overlays::Overlay, voice::VoicePanel};
use gpui::{Bounds, Pixels};
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

#[gpui::test]
fn voice_hides_terminal_scroll_and_leaves_webviews_cut_out_under_it(cx: &mut TestAppContext) {
    let (view, cx, pane) = attached_terminal(cx);
    view.update(cx, |model, cx| {
        let (theme, metrics) = (model.theme.clone(), model.metrics);
        let panel = cx.new(|cx| {
            VoicePanel::with_recorder(Err("no recorder".into()), false, theme, metrics, cx)
        });

        model.voice.view = Some(panel.clone());
        assert!(!terminal_scroll_shown(model, pane, cx));
        let coverage = model.coverage();
        assert!(!coverage.hides_webviews() && !coverage.hides_spinners());
        assert!(coverage.blocks_pane_drag());

        model.voice.view = None;
        model.voice.closing = Some(panel);
        assert!(!terminal_scroll_shown(model, pane, cx));
        assert!(model.coverage().blocks_pane_drag());

        model.voice.closing = None;
        assert!(terminal_scroll_shown(model, pane, cx));
    });
}

/// Draws the window and returns the rectangles marked over native views.
fn drawn_marks(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> Vec<Bounds<Pixels>> {
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    view.update(cx, |model, _| {
        let marks = model.webviews.occlusions.borrow();
        marks.iter().map(|(_, bounds)| *bounds).collect()
    })
}

type Open = Box<dyn Fn(&mut AppModel, &mut Window, &mut Context<AppModel>)>;

#[gpui::test]
fn overlays_mark_exactly_when_their_variant_says(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    assert_eq!(drawn_marks(&view, cx), Vec::new());
    let project = view.update(cx, |model, _| model.state.home().id);
    let at = gpui::point(px(200.0), px(200.0));
    let cases: [(&str, Open); 12] = [
        ("updates", Box::new(AppModel::toggle_update_popover)),
        ("server", Box::new(AppModel::toggle_server_popover)),
        ("remote servers", Box::new(AppModel::toggle_remote_popover)),
        (
            "agents",
            Box::new(|model, _, _| model.overlay = Some(Overlay::Agents)),
        ),
        ("proxy", Box::new(AppModel::open_proxy_popover)),
        (
            "proxy without status bar",
            Box::new(|model, window, cx| {
                model.appearance.status_bar_visible = false;
                model.open_proxy_popover(window, cx);
            }),
        ),
        (
            "menu",
            Box::new(move |model, window, cx| {
                let item = Item::action("Item", Command::AgentsPanel(true));
                model.open_menu(vec![item], at, window, cx);
            }),
        ),
        (
            "project editor",
            Box::new(move |model, window, cx| {
                model.open_project_editor(project, Field::Name, at, window, cx);
            }),
        ),
        (
            "project icons",
            Box::new(move |model, window, cx| model.open_project_icons(project, at, window, cx)),
        ),
        (
            "pair form",
            Box::new(|model, _, cx| model.open_pair_form(None, cx)),
        ),
        (
            "server form",
            Box::new(|model, _, cx| model.open_server_form(None, cx)),
        ),
        (
            "sessions",
            Box::new(move |model, window, cx| model.open_session_picker(project, window, cx)),
        ),
    ];
    for (name, open) in cases {
        let marked = view.update_in(cx, |model, window, cx| {
            model.overlay = None;
            model.appearance.status_bar_visible = true;
            open(model, window, cx);
            let overlay = model.overlay.as_ref().expect(name);
            overlay.mark(model.appearance.status_bar_visible).is_some()
        });
        let marks = drawn_marks(&view, cx);
        let spinners_hidden = view.update(cx, |model, _| model.coverage().hides_spinners());
        assert_eq!(!marks.is_empty(), marked, "{name} marks {marks:?}");
        assert_eq!(spinners_hidden, !marked, "{name} hides spinners");
    }
}
