use gpui::prelude::FluentBuilder;
use gpui::{
    Context, EventEmitter, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Task, Window, actions, div, px,
};
use muxy_ui::{
    components::{ButtonInteraction, SymbolGlyph},
    theme::{Metrics, Theme},
    voice::{Phase, Recorder, Snapshot},
};

actions!(voice, [Finish, Cancel, Pause]);
pub(crate) const TRANSITION: std::time::Duration = std::time::Duration::from_millis(200);

#[derive(Clone)]
pub(crate) enum VoiceEvent {
    Cancel,
    Finish(String),
}

pub(crate) struct VoicePanel {
    focus: FocusHandle,
    recorder: Option<Recorder>,
    snapshot: Snapshot,
    auto_send: bool,
    theme: Theme,
    metrics: Metrics,
    _poll: Task<()>,
    visibility: muxy_ui::motion::VisibilityMotion,
    height: std::rc::Rc<std::cell::Cell<gpui::Pixels>>,
    closing: bool,
}

impl EventEmitter<VoiceEvent> for VoicePanel {}

impl VoicePanel {
    pub(crate) fn new(
        language: String,
        auto_send: bool,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let (recorder, snapshot) = match Recorder::start(language) {
            Ok(recorder) => (Some(recorder), Snapshot::default()),
            Err(error) => (
                None,
                Snapshot {
                    phase: Phase::Failed,
                    error: Some(error),
                    ..Snapshot::default()
                },
            ),
        };
        let poll = cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(80))
                    .await;
                let active = view
                    .update(cx, |view, cx| {
                        let Some(recorder) = &view.recorder else {
                            return false;
                        };
                        view.snapshot = recorder.snapshot();
                        if view.snapshot.phase == Phase::Failed {
                            view.recorder = None;
                        }
                        cx.notify();
                        view.recorder.is_some()
                    })
                    .unwrap_or(false);
                if !active {
                    break;
                }
            }
        });
        Self {
            focus: cx.focus_handle(),
            recorder,
            snapshot,
            auto_send,
            theme,
            metrics,
            _poll: poll,
            visibility: muxy_ui::motion::VisibilityMotion::showing(
                TRANSITION,
                cx.background_executor().clone(),
            ),
            height: std::rc::Rc::new(std::cell::Cell::new(px(200.0))),
            closing: false,
        }
    }

    pub(crate) fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.recorder = None;
        self.closing = true;
        self.visibility.hide();
        cx.notify();
    }

    pub(crate) fn appearance(&mut self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        self.theme = theme;
        self.metrics = metrics;
        cx.notify();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        if self.snapshot.phase == Phase::Starting {
            return;
        }
        let Some(recorder) = self.recorder.take() else {
            return;
        };
        let transcript = recorder.finish();
        if transcript.is_empty() {
            self.snapshot.phase = Phase::Failed;
            self.snapshot.error =
                Some("No speech detected. Try again or press Esc to close.".into());
            cx.notify();
            return;
        }
        cx.emit(VoiceEvent::Finish(transcript));
    }

    fn pause(&mut self, cx: &mut Context<Self>) {
        if let Some(recorder) = &self.recorder {
            recorder.pause(self.snapshot.phase != Phase::Paused);
        }
        cx.notify();
    }

    fn button(
        &self,
        id: &'static str,
        symbol: &'static str,
        color: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id(id)
            .debug_selector(move || id.into())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(self.metrics.control_medium())
            .rounded(self.metrics.radius_md())
            .bg(self.theme.surface)
            .child(SymbolGlyph::new(
                symbol,
                self.metrics.font_footnote(),
                color,
            ))
            .button_interaction(cx.listener(move |view, _, _, cx| match id {
                "voice-cancel" => cx.emit(VoiceEvent::Cancel),
                "voice-pause" => view.pause(cx),
                _ => view.finish(cx),
            }))
            .into_any_element()
    }

    fn main_panel(&self, window: &Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let m = self.metrics;
        let paused = self.snapshot.phase == Phase::Paused;
        let elapsed = self.snapshot.elapsed.as_secs();
        let pulse = if paused {
            1.0
        } else {
            0.7 - 0.3 * (self.snapshot.elapsed.as_secs_f32() * std::f32::consts::PI / 0.8).cos()
        };
        let mut meter = div()
            .flex()
            .flex_1()
            .items_center()
            .gap(m.scaled(3.0))
            .h(m.scaled(24.0));
        for index in 0_u8..11 {
            let curve = 0.5 + 0.5 * (f32::from(index) / 10.0 * std::f32::consts::PI).sin();
            meter = meter.child(
                div()
                    .flex_1()
                    .h(m.scaled(10.0 + 14.0 * curve))
                    .rounded_full()
                    .bg(
                        if !paused && self.snapshot.level >= f32::from(index + 1) / 11.0 {
                            self.theme.accent
                        } else {
                            self.theme.surface
                        },
                    ),
            );
        }
        div()
            .id("voice-recording-controls")
            .debug_selector(|| "voice-recording-controls".into())
            .flex()
            .items_center()
            .gap(m.spacing5())
            .w(m.scaled(280.0))
            .px(m.spacing6())
            .py(m.spacing5())
            .rounded(m.radius_lg())
            .bg(self.theme.bg)
            .relative()
            .child(outline(
                m.radius_lg(),
                if self.focus.is_focused(window) {
                    self.theme.accent.opacity(0.6)
                } else {
                    self.theme.border
                },
            ))
            .shadow(muxy_ui::theme::Elevation::Elevated.shadow(self.theme.bg))
            .child(
                div()
                    .flex_none()
                    .size(m.icon_sm())
                    .rounded_full()
                    .bg(if paused {
                        self.theme.warning
                    } else {
                        self.theme.diff_remove
                    })
                    .opacity(pulse),
            )
            .child(
                div()
                    .flex_none()
                    .w(m.scaled(60.0))
                    .font_family(".AppleSystemUIFontMonospaced")
                    .font_weight(FontWeight::MEDIUM)
                    .text_size(m.font_emphasis())
                    .child(format!("{:02}:{:02}", elapsed / 60, elapsed % 60)),
            )
            .child(meter)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(m.spacing3())
                    .child(self.button("voice-cancel", "xmark", self.theme.fg_muted, cx))
                    .child(self.button(
                        "voice-pause",
                        if paused { "play.fill" } else { "pause.fill" },
                        self.theme.fg,
                        cx,
                    ))
                    .child(self.button("voice-finish", "circle.fill", self.theme.diff_remove, cx)),
            )
            .into_any_element()
    }

    fn hints(&self, window: &Window) -> gpui::AnyElement {
        let m = self.metrics;
        let mut hints = div()
            .flex()
            .gap(m.spacing5())
            .px(m.spacing4())
            .py(m.scaled(3.0))
            .rounded_full()
            .bg(self.theme.bg.opacity(0.85))
            .relative()
            .child(outline(m.scaled(100.0), self.theme.border))
            .text_size(m.font_caption())
            .line_height(m.scaled(13.0))
            .text_color(self.theme.fg_dim)
            .opacity(if self.focus.is_focused(window) {
                1.0
            } else {
                0.55
            });
        for (key, label) in [
            ("⎋", "Cancel"),
            (
                "Space",
                if self.snapshot.phase == Phase::Paused {
                    "Resume"
                } else {
                    "Pause"
                },
            ),
            ("⏎", if self.auto_send { "Send" } else { "Insert" }),
        ] {
            hints = hints.child(
                div()
                    .flex()
                    .gap(m.spacing1())
                    .child(
                        div()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.theme.fg_muted)
                            .child(key),
                    )
                    .child(label),
            );
        }
        hints.into_any_element()
    }
}

fn outline(radius: gpui::Pixels, color: gpui::Hsla) -> impl IntoElement {
    div()
        .absolute()
        .top(px(-0.5))
        .left(px(-0.5))
        .right(px(-0.5))
        .bottom(px(-0.5))
        .rounded(radius)
        .border_1()
        .border_color(color)
}

impl Focusable for VoicePanel {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for VoicePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let progress = self.visibility.sample(window);
        let height = self.height.clone();
        let m = self.metrics;
        let error = self.snapshot.error.as_ref();
        let text = error.cloned().unwrap_or_else(|| {
            if self.snapshot.transcript.is_empty() {
                "Listening...".into()
            } else {
                self.snapshot.transcript.clone()
            }
        });
        let active =
            (self.recorder.is_some() || self.closing) && self.snapshot.phase != Phase::Starting;
        div()
            .id("voice-recording-panel")
            .debug_selector(|| "voice-recording-panel".into())
            .relative()
            .top(self.height.get() * (1.0 - progress))
            .opacity(progress)
            .child(
                gpui::canvas(
                    move |bounds, _, _| height.set(bounds.size.height),
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .key_context("VoiceRecording")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .items_center()
            .gap(m.spacing3())
            .pb(m.scaled(48.0))
            .text_color(self.theme.fg)
            .on_action(cx.listener(|view, _: &Finish, _, cx| {
                view.finish(cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|_, _: &Cancel, _, cx| {
                cx.emit(VoiceEvent::Cancel);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|view, _: &Pause, _, cx| {
                view.pause(cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .w(m.scaled(280.0))
                    .px(m.spacing5())
                    .py(m.spacing3())
                    .rounded(m.radius_md())
                    .bg(self.theme.surface)
                    .text_size(m.font_footnote())
                    .line_height(m.scaled(14.0))
                    .text_color(if error.is_some() {
                        self.theme.diff_remove
                    } else if self.snapshot.transcript.is_empty() {
                        self.theme.fg_dim
                    } else {
                        self.theme.fg_muted
                    })
                    .when(error.is_some(), |chip| {
                        chip.line_clamp(3).child(text.clone())
                    })
                    .when(error.is_none(), |chip| {
                        chip.child(muxy_ui::transcript::Transcript::new(text, 4))
                    }),
            )
            .when(active, |panel| {
                panel
                    .child(self.main_panel(window, cx))
                    .child(self.hints(window))
            })
            .when(error.is_some(), |panel| {
                panel.child(
                    div()
                        .id("voice-error-close")
                        .px(m.spacing5())
                        .py(m.spacing3())
                        .rounded_full()
                        .bg(self.theme.surface)
                        .relative()
                        .child(outline(m.scaled(100.0), self.theme.border))
                        .text_size(m.font_footnote())
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Close")
                        .button_interaction(cx.listener(|_, _, _, cx| cx.emit(VoiceEvent::Cancel))),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext, size};
    use muxy_ui::voice::SimulatedRecorder;

    #[gpui::test]
    fn recording_pause_finish_and_dimensions(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
        });
        let (view, cx): (_, &mut VisualTestContext) = cx.add_window_view(|window, cx| {
            let view = VoicePanel::new(
                String::new(),
                false,
                Theme::from_scheme(&muxy_ui::theme::ColorScheme::default()),
                Metrics::new(1.0),
                cx,
            );
            view.focus_handle(cx).focus(window);
            view
        });
        let snapshot = Snapshot {
            phase: Phase::Recording,
            transcript: "test transcript".into(),
            elapsed: std::time::Duration::from_secs(12),
            level: 0.6,
            error: None,
        };
        let (recorder, simulated) = SimulatedRecorder::new(snapshot.clone());
        view.update(cx, |view, cx| {
            view.recorder = Some(recorder);
            view.snapshot = snapshot;
            cx.notify();
        });
        cx.simulate_resize(size(px(600.0), px(400.0)));
        cx.run_until_parked();
        let bounds = cx.debug_bounds("voice-recording-controls").unwrap();
        assert_eq!(bounds.size.width, px(280.0));
        cx.simulate_keystrokes("space");
        assert!(simulated.paused());
        cx.simulate_keystrokes("enter");
        assert!(simulated.cancelled());
        view.read_with(cx, |view, _| assert!(view.recorder.is_none()));
    }
}
