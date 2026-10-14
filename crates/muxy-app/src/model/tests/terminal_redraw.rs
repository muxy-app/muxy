use super::*;
use muxy_app_core::settings::AppLayout;
use muxy_protocol::{ChannelId, Row, Run, ScreenFrame, Style};

struct TerminalHost {
    pane: PaneView,
    focus_requested: bool,
}

impl gpui::Render for TerminalHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        if std::mem::take(&mut self.focus_requested) {
            self.pane.focus(window, cx);
        }
        self.pane.element(cx)
    }
}

#[gpui::test]
fn focused_terminal_redraws_after_cache_replay(cx: &mut TestAppContext) {
    let (host, cx) = cx.add_window_view(|_, cx| {
        let pane = cx.new(|cx| {
            TerminalPane::new(
                Palette::new(true),
                muxy_app_core::settings::TerminalSettings::default(),
                cx,
            )
        });
        pane.update(cx, |pane, cx| {
            let mut attached = attachment();
            attached.grid.cursor.visible = false;
            pane.attach(ServerId::local(), attached, cx);
        });
        TerminalHost {
            pane: PaneView {
                _subscription: cx.subscribe(&pane, |_, _, _: &PaneEvent, _| {}),
                view: pane,
            },
            focus_requested: true,
        }
    });
    cx.run_until_parked();
    let pane = host.read_with(cx, |host, _| host.pane.view.clone());
    host.update(cx, |host, cx| {
        host.focus_requested = true;
        pane.update(cx, |_, cx| cx.notify());
        cx.notify();
    });
    cx.run_until_parked();
    let renders = pane.read_with(cx, |pane, _| pane.render_count);
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert_eq!(pane.read_with(cx, |pane, _| pane.render_count), renders);
    pane.update(cx, |pane, cx| {
        let grid = pane.grid.as_ref().expect("grid");
        pane.apply(
            &ScreenFrame {
                size: grid.size,
                seq: 1,
                reset: false,
                rows: vec![Row {
                    index: 0,
                    runs: vec![Run {
                        text: "new output".into(),
                        width: 10,
                        style: Style::default(),
                    }],
                }],
                cursor: grid.cursor,
                modes: grid.modes,
                graphics: None,
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert_eq!(
        pane.read_with(cx, |pane, _| pane.grid.as_ref().expect("grid").rows[0][0]
            .text
            .clone()),
        "new output"
    );
    assert!(pane.read_with(cx, |pane, _| pane.render_count) > renders);
}

#[gpui::test]
fn terminal_output_redraws_after_cached_idle_frames(cx: &mut TestAppContext) {
    for layout in [
        AppLayout::ProjectFocused,
        AppLayout::TabFocused,
        AppLayout::AgentsFocused,
    ] {
        let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
        boot.settings.appearance.layout = layout;
        boot.settings.appearance.sidebar_expanded = false;
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        let pane = view.update(cx, |model, cx| {
            model.new_tab(cx);
            model.servers.local.connection = ConnectionState::Ready;
            let pane = model.grids[&model.active_pane().expect("pane")]
                .view
                .clone();
            pane.update(cx, |pane, cx| {
                let mut attached = attachment();
                attached.grid.cursor.visible = false;
                pane.attach(ServerId::local(), attached, cx);
            });
            pane
        });
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert!(model.error.is_none(), "{:?}", model.error);
        });
        view.update(cx, |model, cx| {
            model.focus_requested = true;
            pane.update(cx, |_, cx| cx.notify());
            cx.notify();
        });
        cx.run_until_parked();
        for seq in 1..=5 {
            let renders = pane.read_with(cx, |pane, _| pane.render_count);
            let sidebar = view.read_with(cx, |model, cx| model.sidebar_view.read(cx).render_count);
            cx.update(|window, cx| window.draw(cx).clear());
            cx.run_until_parked();
            assert_eq!(
                pane.read_with(cx, |pane, _| pane.render_count),
                renders,
                "idle cache reuse: {layout:?}"
            );
            assert_eq!(
                view.read_with(cx, |model, cx| model.sidebar_view.read(cx).render_count),
                sidebar,
                "sidebar cache: {layout:?}"
            );
            view.update(cx, |model, cx| {
                let grid = pane.read(cx).grid.as_ref().expect("grid");
                model.receive_event(
                    ServerId::local(),
                    ClientEvent::Frame {
                        channel: ChannelId(1),
                        frame: ScreenFrame {
                            size: grid.size,
                            seq,
                            reset: false,
                            rows: vec![Row {
                                index: 0,
                                runs: vec![Run {
                                    text: format!("output {seq}"),
                                    width: 8,
                                    style: Style::default(),
                                }],
                            }],
                            cursor: grid.cursor,
                            modes: grid.modes,
                            graphics: None,
                        },
                    },
                    cx,
                );
            });
            cx.run_until_parked();
            view.read_with(cx, |model, _| {
                assert!(model.error.is_none(), "{:?}", model.error);
            });
            assert!(
                pane.read_with(cx, |pane, _| pane.render_count) > renders,
                "output must redraw without input: {layout:?}, frame {seq}"
            );
        }
    }
}
