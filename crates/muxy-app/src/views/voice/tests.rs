use super::*;
use gpui::{Entity, TestAppContext, VisualTestContext};
use muxy_ui::voice::SimulatedRecorder;
use std::{cell::RefCell, rc::Rc, time::Duration};

type Finished = Rc<RefCell<Vec<String>>>;

fn panel(
    snapshot: Snapshot,
    cx: &mut TestAppContext,
) -> (
    Entity<VoicePanel>,
    SimulatedRecorder,
    Finished,
    &mut VisualTestContext,
) {
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    let (recorder, simulated) = SimulatedRecorder::new(snapshot);
    let (view, cx) = cx.add_window_view(|window, cx| {
        let view = VoicePanel::with_recorder(
            Ok(recorder),
            false,
            Theme::from_scheme(&muxy_ui::theme::ColorScheme::default()),
            Metrics::new(1.0),
            cx,
        );
        view.focus.focus(window);
        view
    });
    let finished = Finished::default();
    cx.update(|_, cx| {
        let finished = finished.clone();
        cx.subscribe(&view, move |_, event, _| {
            if let VoiceEvent::Finish(transcript) = event {
                finished.borrow_mut().push(transcript.clone());
            }
        })
        .detach();
    });
    poll(cx);
    (view, simulated, finished, cx)
}

fn recording(transcript: &str) -> Snapshot {
    Snapshot {
        phase: Phase::Recording,
        transcript: transcript.into(),
        ..Snapshot::default()
    }
}

fn poll(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(80));
    cx.run_until_parked();
}

#[gpui::test]
fn finish_waits_for_the_final_transcript(cx: &mut TestAppContext) {
    let (_view, simulated, finished, cx) = panel(recording("hello"), cx);
    cx.simulate_keystrokes("enter");
    poll(cx);
    assert!(simulated.finishing());
    assert!(!simulated.cancelled());
    assert!(finished.borrow().is_empty());
    simulated.update(Snapshot {
        phase: Phase::Finished,
        transcript: "hello world".into(),
        ..Snapshot::default()
    });
    poll(cx);
    assert_eq!(*finished.borrow(), ["hello world"]);
    assert!(simulated.cancelled());
    poll(cx);
    assert_eq!(*finished.borrow(), ["hello world"]);
}

#[gpui::test]
fn dismiss_during_transcription_cannot_deliver_a_late_result(cx: &mut TestAppContext) {
    let (view, simulated, finished, cx) = panel(
        Snapshot {
            phase: Phase::Transcribing,
            ..Snapshot::default()
        },
        cx,
    );
    view.update(cx, VoicePanel::dismiss);
    simulated.update(Snapshot {
        phase: Phase::Finished,
        transcript: "Late text".into(),
        ..Snapshot::default()
    });
    poll(cx);
    assert!(simulated.cancelled());
    assert!(finished.borrow().is_empty());
}

#[gpui::test]
fn finish_without_speech_reports_it_instead_of_inserting(cx: &mut TestAppContext) {
    let (view, simulated, finished, cx) = panel(recording(""), cx);
    cx.simulate_keystrokes("enter");
    simulated.update(Snapshot {
        phase: Phase::Finished,
        ..Snapshot::default()
    });
    poll(cx);
    assert!(finished.borrow().is_empty());
    assert!(simulated.cancelled());
    view.read_with(cx, |view, _| {
        assert_eq!(view.snapshot.phase, Phase::Failed);
    });
}

#[gpui::test]
fn finish_is_ignored_until_recording_starts(cx: &mut TestAppContext) {
    let (_view, simulated, finished, cx) = panel(Snapshot::default(), cx);
    cx.simulate_keystrokes("enter");
    poll(cx);
    assert!(!simulated.finishing());
    assert!(!simulated.cancelled());
    assert!(finished.borrow().is_empty());
}

#[gpui::test]
fn failure_is_reported_even_after_finish_was_requested(cx: &mut TestAppContext) {
    let (view, simulated, finished, cx) = panel(recording("hello"), cx);
    cx.simulate_keystrokes("enter");
    simulated.update(Snapshot {
        phase: Phase::Failed,
        transcript: "hello".into(),
        error: Some("Microphone recording stopped. Try again.".into()),
        ..Snapshot::default()
    });
    poll(cx);
    assert!(finished.borrow().is_empty());
    assert!(simulated.cancelled());
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.snapshot.error.as_deref(),
            Some("Microphone recording stopped. Try again.")
        );
    });
}
