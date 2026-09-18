use super::*;
use gpui::{TestAppContext, VisualTestContext, size};
use muxy_ui::voice::{Phase, SimulatedRecorder, Snapshot};

struct ComposerHost(Entity<Composer>);

impl Render for ComposerHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use gpui::{ParentElement, Styled};
        gpui::div()
            .relative()
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .child(self.0.clone())
    }
}

#[gpui::test]
fn editor_fills_available_space_in_every_composer_mode(cx: &mut TestAppContext) {
    use gpui::{EntityInputHandler, Modifiers, point};
    use muxy_app_core::settings::ComposerPresentation;

    let (host, cx): (_, &mut VisualTestContext) = cx.add_window_view(|_, cx| {
        ComposerHost(cx.new(|cx| {
            Composer::new(
                ComposerDraft::default(),
                ComposerSettings::default(),
                Theme::from_scheme(&muxy_ui::theme::ColorScheme::default()),
                Metrics::new(1.0),
                cx,
            )
        }))
    });
    let view = host.read_with(cx, |host, _| host.0.clone());
    cx.executor().advance_clock(TRANSITION);
    for presentation in [ComposerPresentation::Panel, ComposerPresentation::Floating] {
        for position in [ComposerPosition::Right, ComposerPosition::Bottom] {
            for pinned in [false, true] {
                view.update(cx, |view, cx| {
                    view.settings.presentation = presentation;
                    view.settings.position = position;
                    view.settings.pinned = pinned;
                    cx.notify();
                });
                for dimensions in [size(px(600.0), px(400.0)), size(px(900.0), px(700.0))] {
                    cx.simulate_resize(dimensions);
                    cx.run_until_parked();
                    let editor = cx.debug_bounds("composer-editor").expect("editor");
                    assert!(editor.size.width > px(250.0));
                    assert!(
                        editor.size.height > px(100.0),
                        "{editor:?} in {presentation:?}/{position:?}"
                    );
                    let input = view.read_with(cx, |view, _| view.input.clone());
                    for text in ["", "first line\nsecond line\nthird line\nfourth line"] {
                        input.update(cx, |input, cx| input.set_text(text, cx));
                        cx.run_until_parked();
                        for (point, expected) in [
                            (editor.origin + point(px(3.0), px(13.0)), 0),
                            (
                                point(editor.right() - px(3.0), editor.bottom() - px(13.0)),
                                text.len(),
                            ),
                        ] {
                            let index = cx.update(|window, cx| {
                                input.update(cx, |input, cx| {
                                    input.character_index_for_point(point, window, cx)
                                })
                            });
                            assert_eq!(
                                index,
                                Some(expected),
                                "editor does not fill {editor:?} in {presentation:?}/{position:?}, pinned={pinned}"
                            );
                            cx.update(|window, _| window.blur());
                            cx.simulate_click(point, Modifiers::default());
                            cx.update(|window, cx| {
                                assert!(input.focus_handle(cx).is_focused(window));
                            });
                        }
                    }
                }
            }
        }
    }
}

#[gpui::test]
fn finish_dictation_inserts_without_sending_and_close_cancels(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    let (view, cx): (_, &mut VisualTestContext) = cx.add_window_view(|window, cx| {
        let view = Composer::new(
            ComposerDraft::default(),
            ComposerSettings::default(),
            Theme::from_scheme(&muxy_ui::theme::ColorScheme::default()),
            Metrics::new(1.0),
            cx,
        );
        view.focus_handle(cx).focus(window);
        view
    });
    cx.simulate_resize(size(px(600.0), px(400.0)));
    let snapshot = Snapshot {
        phase: Phase::Recording,
        transcript: "dictated text".into(),
        ..Snapshot::default()
    };
    let (recorder, simulated) = SimulatedRecorder::new(snapshot.clone());
    view.update(cx, |view, _| {
        view.voice = Some(recorder);
        view.voice_snapshot = snapshot;
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.draft.text, "dictated text");
        assert!(!view.recording());
        assert!(!view.sending);
    });
    assert!(simulated.cancelled());
    let (recorder, simulated) = SimulatedRecorder::new(Snapshot::default());
    view.update(cx, |view, cx| {
        view.voice = Some(recorder);
        view.cancel_voice(cx);
    });
    assert!(simulated.cancelled());
}
