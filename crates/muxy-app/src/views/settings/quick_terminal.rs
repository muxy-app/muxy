use super::{Category, Change, SettingsEvent, SettingsView};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div, px,
};
use muxy_core::quick_terminal::QuickTerminalShortcut;
use muxy_ui::controls;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

#[allow(clippy::too_many_lines, reason = "Declarative settings layout")]
pub(super) fn rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let settings = &view.snapshot.settings.quick_terminal;
    let mut rows = Vec::new();
    if view.matches(Category::QuickTerminal, tr_key!("Enable Quick Terminal")) {
        let mut next = settings.clone();
        next.enabled = !next.enabled;
        rows.push(view.row(
            "quick-enabled",
            tr_key!("Enable Quick Terminal"),
            view.toggle(
                "quick-enabled",
                settings.enabled,
                Change::QuickTerminal(next),
                cx,
            ),
        ));
        if !settings.enabled {
            rows.push(view.note(&tr!("The Quick Terminal shortcut listener and shell are off. Your shortcut, size, and appearance settings are preserved."), false).into_any_element());
        }
    }
    if view.matches(Category::QuickTerminal, tr_key!("Open Quick Terminal")) {
        let mut buttons = div().flex().flex_wrap().gap(px(8.0));
        let mut unassigned = settings.clone();
        unassigned.shortcut = QuickTerminalShortcut::Unassigned;
        buttons = buttons.child(
            controls::button(
                view.style(),
                "quick-unassigned",
                &tr!("No Shortcut"),
                true,
                cx.listener(move |_: &mut SettingsView, _, _, cx| {
                    cx.emit(SettingsEvent::Change(Change::QuickTerminal(
                        unassigned.clone(),
                    )));
                }),
            )
            .when(
                settings.shortcut == QuickTerminalShortcut::Unassigned,
                |button| button.border_color(view.theme.accent),
            )
            .debug_selector(|| "settings-quick-unassigned".to_owned()),
        );
        buttons = buttons.child(
            controls::button(
                view.style(),
                "quick-record",
                &if view.quick_recording.is_some() {
                    tr!("Cancel Recording")
                } else {
                    tr!("Record Shortcut…")
                },
                true,
                cx.listener(|view, _, window, cx| view.record_quick_shortcut(window, cx)),
            )
            .debug_selector(|| "settings-quick-record".to_owned()),
        );
        let status = if !settings.enabled {
            tr!("Disabled")
        } else if settings.shortcut == QuickTerminalShortcut::Unassigned {
            tr!("No shortcut assigned")
        } else {
            view.snapshot.quick_shortcut_status.clone().into()
        };
        let label = settings
            .shortcut
            .key_combo()
            .map(muxy_core::quick_terminal::keys::KeyCombo::display);
        rows.push(
            view.row(
                "quick-shortcut",
                tr_key!("Open Quick Terminal"),
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(buttons)
                    .children(label.map(|label| div().child(label)))
                    .child(view.note(&status, false))
                    .into_any_element(),
            ),
        );
    }
    if view.matches(Category::QuickTerminal, tr_key!("Terminal size")) {
        let mut next = settings.clone();
        next.width = 720;
        next.height = 430;
        rows.push(
            view.row(
                "quick-size",
                tr_key!("Terminal size"),
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(tr!("Width"))
                    .child(view.field("quick-width"))
                    .child(tr!("Height"))
                    .child(view.field("quick-height"))
                    .child(controls::button(
                        view.style(),
                        "quick-size-reset",
                        &tr!("Reset"),
                        true,
                        cx.listener(move |_: &mut SettingsView, _, _, cx| {
                            cx.emit(SettingsEvent::Change(Change::QuickTerminal(next.clone())));
                        }),
                    ))
                    .into_any_element(),
            ),
        );
    }
    for (id, label, value, max) in [
        (
            "quick-transparency",
            tr_key!("Terminal transparency"),
            settings.transparency,
            55.0,
        ),
        (
            "quick-blur",
            tr_key!("Background vibrancy"),
            settings.blur,
            100.0,
        ),
    ] {
        if view.matches(Category::QuickTerminal, label) {
            let slider = controls::slider(
                view.style(),
                id,
                f32::from(value),
                (0.0, max),
                cx.listener(move |view, grab: &controls::Grab, _, cx| {
                    view.quick_slider = Some((id, grab.bounds));
                    view.move_quick_slider(id, grab.bounds, grab.position, cx);
                }),
            );
            let mut control = div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(slider)
                .child(tr!("%lld%%", value));
            if id == "quick-blur" {
                let mut next = settings.clone();
                next.transparency = 18;
                next.blur = 70;
                control = control.child(controls::button(
                    view.style(),
                    "quick-appearance-reset",
                    &tr!("Reset"),
                    true,
                    cx.listener(move |_: &mut SettingsView, _, _, cx| {
                        cx.emit(SettingsEvent::Change(Change::QuickTerminal(next.clone())));
                    }),
                ));
            }
            rows.push(view.row(id, label, control.into_any_element()));
        }
    }
    rows
}

impl SettingsView {
    pub(crate) fn show_quick_terminal(&mut self, cx: &mut Context<Self>) {
        self.category = Category::QuickTerminal;
        self.section = None;
        self.query.clear();
        self.search.update(cx, |search, cx| search.set_text("", cx));
        self.results.reset();
        cx.notify();
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(super) fn move_quick_slider(
        &mut self,
        id: &'static str,
        bounds: gpui::Bounds<gpui::Pixels>,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let mut next = self.snapshot.settings.quick_terminal.clone();
        let fraction = controls::fraction_at(bounds, position);
        if id == "quick-transparency" {
            next.transparency = (fraction * 55.0).round() as u8;
        } else {
            next.blur = (fraction * 100.0).round() as u8;
        }
        if next != self.snapshot.settings.quick_terminal {
            cx.emit(SettingsEvent::Change(Change::QuickTerminal(next)));
        }
    }

    fn record_quick_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.quick_recording.take().is_some() {
            self.results.dirty = true;
            cx.notify();
            return;
        }
        self.focus.focus(window);
        match muxy_ui::quick_terminal::start_shortcut_recording() {
            Ok((recorder, receiver)) => {
                let task = cx.spawn(async move |view, cx| {
                    if let Ok(event) = receiver.recv().await {
                        let _ = view.update(cx, |view, cx| {
                            use muxy_ui::quick_terminal::ShortcutRecordingEvent;
                            view.quick_recording = None;
                            match event {
                                ShortcutRecordingEvent::Captured(capture) => {
                                    let mut settings =
                                        view.snapshot.settings.quick_terminal.clone();
                                    settings.shortcut = QuickTerminalShortcut::KeyCombo {
                                        key_combo: capture.combo,
                                        virtual_key_code: capture.virtual_key_code,
                                    };
                                    cx.emit(SettingsEvent::Change(Change::QuickTerminal(settings)));
                                }
                                ShortcutRecordingEvent::Rejected(error) => {
                                    view.errors.insert("quick-shortcut".into(), error);
                                }
                                ShortcutRecordingEvent::Cancelled => {}
                            }
                            view.results.dirty = true;
                            cx.notify();
                        });
                    }
                });
                self.quick_recording = Some((recorder, task));
            }
            Err(error) => {
                self.errors.insert("quick-shortcut".into(), error);
            }
        }
        self.results.dirty = true;
        cx.notify();
    }
}
