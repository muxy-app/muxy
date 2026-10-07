use super::*;
use gpui::{TestAppContext, VisualTestContext, size};
use muxy_ui::voice::{Phase, SimulatedRecorder, Snapshot};

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
