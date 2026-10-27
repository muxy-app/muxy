use super::*;
use gpui::{TestAppContext, VisualTestContext, size};
use muxy_ui::voice::{Phase, SimulatedRecorder, Snapshot};

fn composer(cx: &mut TestAppContext) -> (Entity<Composer>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    let (view, cx): (_, &mut VisualTestContext) = cx.add_window_view(|window, cx| {
        let view = Composer::new(
            ComposerDraft {
                text: "Existing draft. ".into(),
                ..ComposerDraft::default()
            },
            ComposerSettings::default(),
            Theme::from_scheme(&muxy_ui::theme::ColorScheme::default()),
            Metrics::new(1.0),
            cx,
        );
        view.focus_handle(cx).focus(window);
        view
    });
    cx.simulate_resize(size(px(600.0), px(400.0)));
    (view, cx)
}

#[gpui::test]
fn finish_dictation_inserts_without_sending_and_close_cancels(cx: &mut TestAppContext) {
    let (view, cx) = composer(cx);
    let (recorder, simulated) = SimulatedRecorder::new(Snapshot {
        phase: Phase::Recording,
        transcript: "dictated".into(),
        ..Snapshot::default()
    });
    view.update(cx, |view, cx| view.record(recorder, cx));
    poll(cx);
    cx.simulate_keystrokes("enter");
    poll(cx);
    assert!(simulated.finishing());
    view.read_with(cx, |view, _| {
        assert_eq!(view.draft.text, "Existing draft. ");
        assert!(view.recording());
    });
    simulated.update(Snapshot {
        phase: Phase::Transcribing,
        ..Snapshot::default()
    });
    poll(cx);
    cx.simulate_keystrokes("enter");
    poll(cx);
    assert!(!simulated.cancelled());
    simulated.update(Snapshot {
        phase: Phase::Finished,
        transcript: "dictated text".into(),
        ..Snapshot::default()
    });
    poll(cx);
    poll(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.draft.text, "Existing draft. dictated text");
        assert!(!view.recording());
        assert!(!view.sending);
    });
    assert!(simulated.cancelled());
    let (recorder, simulated) = SimulatedRecorder::new(Snapshot {
        phase: Phase::Recording,
        ..Snapshot::default()
    });
    view.update(cx, |view, cx| view.record(recorder, cx));
    poll(cx);
    cx.simulate_keystrokes("enter");
    simulated.update(Snapshot {
        phase: Phase::Finished,
        ..Snapshot::default()
    });
    poll(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.draft.text, "Existing draft. dictated text");
        assert!(!view.recording());
    });
    let (recorder, simulated) = SimulatedRecorder::new(Snapshot::default());
    view.update(cx, |view, cx| {
        view.voice = Some(recorder);
        view.cancel_voice(cx);
    });
    assert!(simulated.cancelled());
    let (recorder, simulated) = SimulatedRecorder::new(Snapshot::default());
    view.update(cx, |view, cx| view.record(recorder, cx));
    poll(cx);
    cx.simulate_keystrokes("enter");
    poll(cx);
    assert!(!simulated.finishing());
    assert!(simulated.cancelled());
    view.read_with(cx, |view, _| {
        assert_eq!(view.draft.text, "Existing draft. dictated text");
        assert!(!view.recording());
    });
    let (recorder, simulated) = SimulatedRecorder::new(Snapshot {
        phase: Phase::Transcribing,
        ..Snapshot::default()
    });
    view.update(cx, |view, cx| view.record(recorder, cx));
    poll(cx);
    view.update(cx, Composer::cancel_voice);
    simulated.update(Snapshot {
        phase: Phase::Finished,
        transcript: "Late text".into(),
        ..Snapshot::default()
    });
    poll(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.draft.text, "Existing draft. dictated text");
    });
}

fn poll(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(80));
    cx.run_until_parked();
}
