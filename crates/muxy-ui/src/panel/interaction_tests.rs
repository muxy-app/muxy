use super::*;
use gpui::{Context, Render, TestAppContext, VisualTestContext, point, size};

struct PanelTestHost {
    placement: PanelPlacement,
    dimension: f32,
    resize: PanelResizeState,
    scale: f32,
}

impl Render for PanelTestHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.weak_entity();
        div().relative().flex().size_full().child(PanelFrame::new(
            self.placement.clone(),
            PanelSizing::new(
                &self.placement,
                self.dimension,
                PanelSizeBounds::new(100.0, 500.0),
                self.resize.clone(),
            ),
            div()
                .debug_selector(|| "panel-test-chrome".into())
                .h(px(33.0))
                .flex_none(),
            div()
                .id("panel-test-content")
                .debug_selector(|| "panel-test-content".into())
                .size_full()
                .on_mouse_move(|_, _, cx| cx.stop_propagation()),
            move |dimension, _, cx| {
                let _ = owner.update(cx, |host, cx| {
                    host.dimension = dimension;
                    cx.notify();
                });
            },
            PanelStyle::new(
                Theme::from_scheme(&crate::theme::ColorScheme::default()),
                Metrics::new(self.scale),
            ),
        ))
    }
}

#[gpui::test]
fn resize_grip_stays_inside_panel_and_tracks_drag_across_content(cx: &mut TestAppContext) {
    let (host, cx): (_, &mut VisualTestContext) = cx.add_window_view(|_, _| PanelTestHost {
        placement: PanelPlacement::new("test", PanelPosition::Right, PanelMode::Pinned),
        dimension: 300.0,
        resize: PanelResizeState::default(),
        scale: 1.0,
    });
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for position in [PanelPosition::Right, PanelPosition::Bottom] {
        for mode in [PanelMode::Pinned, PanelMode::Floating] {
            for scale in [1.0, 1.5] {
                host.update(cx, |host, cx| {
                    host.placement.position = position;
                    host.placement.mode = mode;
                    host.dimension = 300.0;
                    host.scale = scale;
                    cx.notify();
                });
                cx.run_until_parked();
                let grip = cx.debug_bounds("panel-resize-test").expect("resize grip");
                let content = cx.debug_bounds("panel-test-content").expect("content");
                let chrome = cx.debug_bounds("panel-test-chrome").expect("chrome");
                assert_eq!(content.left(), chrome.left());
                assert_eq!(content.right(), chrome.right());
                assert_eq!(content.top(), chrome.bottom());
                let start = match position {
                    PanelPosition::Right => {
                        assert_eq!(content.left(), grip.left() + px(1.0));
                        assert_eq!(content.size.width, px(300.0));
                        assert_eq!(content.bottom(), grip.bottom());
                        assert_eq!(
                            grip.size.width,
                            Metrics::new(scale).resize_handle_hit_area()
                        );
                        point(grip.right() - px(1.0), content.center().y)
                    }
                    PanelPosition::Bottom => {
                        assert_eq!(chrome.top(), grip.top() + px(1.0));
                        assert_eq!(chrome.size.height + content.size.height, px(300.0));
                        assert_eq!(content.left(), grip.left());
                        assert_eq!(content.right(), grip.right());
                        assert_eq!(
                            grip.size.height,
                            Metrics::new(scale).resize_handle_hit_area()
                        );
                        point(content.center().x, grip.bottom() - px(1.0))
                    }
                };
                cx.simulate_event(MouseMoveEvent {
                    position: start,
                    ..Default::default()
                });
                cx.simulate_event(MouseDownEvent {
                    position: start,
                    button: MouseButton::Left,
                    click_count: 1,
                    ..Default::default()
                });
                cx.run_until_parked();
                assert!(host.read_with(cx, |host, _| host.resize.is_active()));
                let delta = match position {
                    PanelPosition::Right => point(px(80.0), px(0.0)),
                    PanelPosition::Bottom => point(px(0.0), px(80.0)),
                };
                cx.simulate_event(MouseMoveEvent {
                    position: start + delta,
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                });
                cx.run_until_parked();
                assert_eq!(host.read_with(cx, |host, _| px(host.dimension)), px(220.0));
                cx.simulate_event(MouseUpEvent {
                    position: start - delta,
                    button: MouseButton::Left,
                    ..Default::default()
                });
                cx.run_until_parked();
                host.read_with(cx, |host, _| {
                    assert_eq!(px(host.dimension), px(380.0));
                    assert!(!host.resize.is_active());
                });
                cx.simulate_event(MouseMoveEvent {
                    position: start,
                    ..Default::default()
                });
                assert_eq!(host.read_with(cx, |host, _| px(host.dimension)), px(380.0));
            }
        }
    }
}

#[gpui::test]
fn resize_stops_when_mouse_release_was_missed(cx: &mut TestAppContext) {
    let (host, cx) = cx.add_window_view(|_, _| PanelTestHost {
        placement: PanelPlacement::new("test", PanelPosition::Right, PanelMode::Pinned),
        dimension: 300.0,
        resize: PanelResizeState::default(),
        scale: 1.0,
    });
    cx.run_until_parked();
    let start = cx.debug_bounds("panel-resize-test").expect("grip").center();
    cx.simulate_event(MouseDownEvent {
        position: start,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(MouseMoveEvent {
        position: start + point(px(50.0), px(0.0)),
        ..Default::default()
    });
    cx.run_until_parked();
    host.read_with(cx, |host, _| {
        assert!(!host.resize.is_active());
        assert_eq!(px(host.dimension), px(300.0));
    });
}
