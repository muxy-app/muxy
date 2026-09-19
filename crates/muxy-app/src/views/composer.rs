mod layout;
pub(crate) mod menu;

use gpui::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Subscription, Task, Window, actions, px,
};
use muxy_app_core::{
    composer::ComposerDraft,
    settings::{ComposerPosition, ComposerSettings},
};
use muxy_ui::{
    panel::{
        PanelMode, PanelPlacement, PanelPosition, PanelResizeState, PanelSizeBounds, PanelSizing,
    },
    text_input::{InputEvent, InputStyle, TextInput},
    theme::{Metrics, Theme},
};

actions!(composer, [Submit, Insert, Close, ToggleVoice]);
pub(crate) const PANEL: &str = "builtin:composer";
pub(crate) const TRANSITION: std::time::Duration = std::time::Duration::from_millis(150);

#[derive(Clone)]
pub(crate) enum ComposerEvent {
    Changed,
    Submit(bool),
    Attach,
    Files(Vec<std::path::PathBuf>),
    Image(Vec<u8>),
    Close,
    Menu,
    Preferences(ComposerSettings),
}

pub(crate) struct Composer {
    pub(crate) input: Entity<TextInput>,
    pub(crate) draft: ComposerDraft,
    pub(crate) settings: ComposerSettings,
    pub(crate) error: Option<String>,
    pub(crate) sending: bool,
    theme: Theme,
    metrics: Metrics,
    resize: PanelResizeState,
    header_focus: [FocusHandle; 6],
    _input_subscription: Subscription,
    voice: Option<muxy_ui::voice::Recorder>,
    voice_snapshot: muxy_ui::voice::Snapshot,
    voice_poll: Option<Task<()>>,
    pub(crate) workspace_inset: f32,
    pub(crate) context_label: String,
    floating_dimensions: Option<gpui::Size<gpui::Pixels>>,
    size_motion: Option<layout::SizeMotion>,
    pub(crate) menu_anchor: std::rc::Rc<std::cell::Cell<gpui::Point<gpui::Pixels>>>,
    pub(crate) visibility: muxy_ui::motion::VisibilityMotion,
    input_style: InputStyle,
    floating_resize: Option<(
        gpui::Point<gpui::Pixels>,
        gpui::Size<gpui::Pixels>,
        layout::ResizeEdge,
    )>,
}

impl EventEmitter<ComposerEvent> for Composer {}

impl Composer {
    pub(crate) fn new(
        draft: ComposerDraft,
        settings: ComposerSettings,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let owner = cx.weak_entity();
        let input = cx.new(|cx| {
            TextInput::new(InputStyle::field(&theme, &metrics), cx)
                .multiline()
                .with_text(draft.text.clone())
                .with_placeholder("Type or speak…")
                .with_placeholder_font(".SystemUIFont")
                .with_paste_delegate(move |_, cx| {
                    let content = muxy_ui::pasteboard::read_content();
                    if matches!(content, Ok(muxy_ui::pasteboard::Content::Text)) {
                        return false;
                    }
                    let _ = owner.update(cx, |view: &mut Self, cx| match content {
                        Ok(muxy_ui::pasteboard::Content::Files(paths)) => {
                            cx.emit(ComposerEvent::Files(paths));
                        }
                        Ok(muxy_ui::pasteboard::Content::Image(bytes)) => {
                            cx.emit(ComposerEvent::Image(bytes));
                        }
                        Err(error) => {
                            view.error = Some(error);
                            cx.notify();
                        }
                        Ok(muxy_ui::pasteboard::Content::Text) => {}
                    });
                    true
                })
        });
        let subscription = cx.subscribe(&input, |view: &mut Self, input, event, cx| {
            if matches!(event, InputEvent::Cancelled) {
                cx.emit(ComposerEvent::Close);
            }
            if matches!(event, InputEvent::Changed) {
                input.read(cx).text().clone_into(&mut view.draft.text);
                cx.emit(ComposerEvent::Changed);
            }
        });
        let input_style = InputStyle::field(&theme, &metrics);
        let mut view = Self {
            input,
            draft,
            settings,
            error: None,
            sending: false,
            theme,
            metrics,
            resize: PanelResizeState::default(),
            header_focus: std::array::from_fn(|_| cx.focus_handle()),
            _input_subscription: subscription,
            voice: None,
            voice_snapshot: muxy_ui::voice::Snapshot::default(),
            voice_poll: None,
            workspace_inset: 0.0,
            context_label: String::new(),
            floating_resize: None,
            floating_dimensions: None,
            size_motion: None,
            menu_anchor: std::rc::Rc::default(),
            visibility: muxy_ui::motion::VisibilityMotion::showing(
                TRANSITION,
                cx.background_executor().clone(),
            ),
            input_style,
        };
        view.refresh_style(cx);
        view
    }

    pub(crate) fn sizing(&self, window: &Window) -> PanelSizing {
        let placement = self.placement();
        let (dimension, maximum, minimum) = if placement.position == PanelPosition::Right {
            (
                self.settings.width,
                f32::from(window.viewport_size().width) - self.workspace_inset - 100.0,
                280.0,
            )
        } else {
            (
                self.settings.height,
                f32::from(window.viewport_size().height) - 160.0,
                120.0,
            )
        };
        PanelSizing::new(
            &placement,
            dimension,
            PanelSizeBounds::new(
                minimum,
                maximum
                    .max(minimum)
                    .min(if placement.position == PanelPosition::Right {
                        800.0
                    } else {
                        600.0
                    }),
            ),
            self.resize.clone(),
        )
    }

    pub(crate) fn placement(&self) -> PanelPlacement {
        PanelPlacement::new(
            PANEL,
            if self.settings.position == ComposerPosition::Right {
                PanelPosition::Right
            } else {
                PanelPosition::Bottom
            },
            if self.settings.pinned {
                PanelMode::Pinned
            } else {
                PanelMode::Floating
            },
        )
    }

    pub(crate) fn appearance(&mut self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        self.theme = theme;
        self.metrics = metrics;
        self.refresh_style(cx);
    }

    pub(crate) fn refresh_style(&mut self, cx: &mut Context<Self>) {
        let mut style = InputStyle::field(&self.theme, &self.metrics);
        style.font_size = px(self.settings.font_size);
        let family = if self.settings.font_family.is_empty()
            || !cx
                .text_system()
                .all_font_names()
                .contains(&self.settings.font_family)
        {
            ".AppleSystemUIFontMonospaced".to_owned()
        } else {
            self.settings.font_family.clone()
        };
        let font = gpui::font(family.clone());
        let id = cx.text_system().resolve_font(&font);
        let height = cx.text_system().ascent(id, style.font_size)
            + px(f32::from(cx.text_system().descent(id, style.font_size)).abs());
        style.line_height = px((f32::from(height) * self.settings.line_height).ceil());
        style.cursor = self.theme.fg;
        style.placeholder = gpui::Hsla {
            a: self.theme.fg_muted.a * 0.55,
            ..self.theme.fg_muted
        };
        self.input_style = style;
        self.input.update(cx, |input, cx| {
            input.set_style(style, cx);
            input.set_font_family(Some(family.into()), cx);
        });
        cx.notify();
    }

    pub(crate) fn set_draft(&mut self, draft: ComposerDraft, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_text(draft.text.clone(), cx));
        self.draft = draft;
        cx.notify();
    }

    pub(crate) fn cancel_voice(&mut self, cx: &mut Context<Self>) {
        self.voice = None;
        self.voice_poll = None;
        self.error = None;
        cx.notify();
    }

    pub(crate) fn recording(&self) -> bool {
        self.voice.is_some()
    }

    fn submit(&self, enter: bool, cx: &mut Context<Self>) {
        if !self.sending && self.voice.is_none() {
            cx.emit(ComposerEvent::Submit(enter));
        }
    }

    pub(crate) fn toggle_voice(&mut self, cx: &mut Context<Self>) {
        if self.sending {
            return;
        }
        if let Some(recorder) = self.voice.take() {
            self.voice_poll = None;
            if self.voice_snapshot.phase == muxy_ui::voice::Phase::Starting {
                drop(recorder);
                cx.notify();
                return;
            }
            let text = recorder.finish();
            if text.is_empty() {
                self.error = Some("No speech detected. Try dictation again.".into());
            } else {
                self.input
                    .update(cx, |input, cx| input.insert_at_selection(&text, cx));
            }
            cx.notify();
            return;
        }
        self.error = None;
        match muxy_ui::voice::Recorder::start(self.settings.language.clone()) {
            Ok(recorder) => self.voice = Some(recorder),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        }
        self.voice_snapshot = muxy_ui::voice::Snapshot::default();
        self.voice_poll = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(80))
                    .await;
                let keep = view
                    .update(cx, |view, cx| {
                        let Some(recorder) = &view.voice else {
                            return false;
                        };
                        view.voice_snapshot = recorder.snapshot();
                        view.error.clone_from(&view.voice_snapshot.error);
                        let active = view.voice_snapshot.phase != muxy_ui::voice::Phase::Failed;
                        if !active {
                            view.voice = None;
                        }
                        cx.notify();
                        active
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn zoom(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.settings.font_size = (self.settings.font_size + delta).clamp(9.0, 32.0);
        self.refresh_style(cx);
        self.preferences(cx);
        cx.stop_propagation();
    }

    fn preferences(&mut self, cx: &mut Context<Self>) {
        cx.emit(ComposerEvent::Preferences(self.settings.clone()));
        cx.notify();
    }

    pub(crate) fn pause_voice(&mut self, cx: &mut Context<Self>) {
        if let Some(voice) = &self.voice {
            voice.pause(self.voice_snapshot.phase != muxy_ui::voice::Phase::Paused);
        }
        cx.notify();
    }
}

impl Focusable for Composer {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_composer(window, cx)
    }
}

#[cfg(test)]
mod tests;
