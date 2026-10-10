use super::pickers::FontSlot;
use super::terminal_lists::{self, BINDING_ACTION};
use super::{Category, Change, PickerKind, SettingsEvent, SettingsView, catalog};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div, px,
};
use muxy_app_core::settings::{
    CellHeight, NewPaneDirectory, PaddingColor, TerminalColor, TerminalSettings,
};
use muxy_ui::controls;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

pub(super) const SECTIONS: &[&str] = &[
    tr_key!("Font"),
    tr_key!("Background"),
    tr_key!("Colors"),
    tr_key!("Cursor"),
    tr_key!("Selection"),
    tr_key!("Scrolling"),
    tr_key!("Input"),
    tr_key!("Key bindings"),
    tr_key!("Migration"),
    tr_key!("Reset"),
];
pub(super) const PICKERS: &[&str] = &[
    "cursor-style",
    "cursor-style-blink",
    "macos-option-as-alt",
    "directory",
    "adjust-cell-height",
    "adjust-cell-width",
    "adjust-cursor-thickness",
    "window-padding-color",
    BINDING_ACTION,
];
const PERCENT_RANGE: (f32, f32) = (-50.0, 200.0);
const PIXEL_RANGE: (f32, f32) = (-5.0, 20.0);
const COLORS: [&str; 6] = [
    "background",
    "foreground",
    "cursor-color",
    "cursor-text",
    "selection-foreground",
    "selection-background",
];
const PALETTE: [&str; 16] = [
    "palette-0",
    "palette-1",
    "palette-2",
    "palette-3",
    "palette-4",
    "palette-5",
    "palette-6",
    "palette-7",
    "palette-8",
    "palette-9",
    "palette-10",
    "palette-11",
    "palette-12",
    "palette-13",
    "palette-14",
    "palette-15",
];

pub(super) fn owns(id: &str) -> bool {
    PALETTE.contains(&id)
        || catalog::setting(id).is_some_and(|setting| setting.category == Category::Terminal)
}

pub(super) fn fields() -> impl Iterator<Item = &'static str> {
    std::iter::once("font-feature").chain(COLORS).chain(PALETTE)
}

pub(super) fn field_values(
    terminal: &TerminalSettings,
) -> impl Iterator<Item = (&'static str, String)> + '_ {
    std::iter::once(("font-feature", terminal.font.feature_list()))
        .chain(
            COLORS
                .into_iter()
                .map(|id| (id, color(terminal, id).map_or_else(String::new, color_text))),
        )
        .chain(PALETTE.into_iter().zip(0_u8..).map(|(id, index)| {
            (
                id,
                terminal
                    .options
                    .palette
                    .get(&index)
                    .map_or_else(String::new, |rgb| format!("#{rgb:06x}")),
            )
        }))
}

fn color(terminal: &TerminalSettings, id: &str) -> Option<TerminalColor> {
    let options = &terminal.options;
    match id {
        "background" => options.background.map(TerminalColor::Rgb),
        "foreground" => options.foreground.map(TerminalColor::Rgb),
        "cursor-color" => options.cursor_color,
        "cursor-text" => options.cursor_text,
        "selection-foreground" => options.selection_foreground,
        _ => options.selection_background,
    }
}

fn color_text(color: TerminalColor) -> String {
    match color {
        TerminalColor::Rgb(rgb) => format!("#{rgb:06x}"),
        other => String::from(other),
    }
}

fn swatch(pane: &SettingsView, color: Option<TerminalColor>) -> AnyElement {
    div()
        .size(px(16.0))
        .flex_none()
        .rounded(px(3.0))
        .border_1()
        .border_color(pane.theme.border)
        .when_some(
            color.and_then(|color| match color {
                TerminalColor::Rgb(rgb) => Some(rgb),
                _ => None,
            }),
            |swatch, rgb| swatch.bg(gpui::rgb(rgb)),
        )
        .into_any_element()
}

pub(super) fn change_button(
    pane: &SettingsView,
    id: &str,
    label: &str,
    enabled: bool,
    change: Change,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    controls::button(
        pane.style(),
        id,
        label,
        enabled,
        cx.listener(move |pane, _, _, cx| {
            pane.recording = None;
            cx.emit(SettingsEvent::Change(change.clone()));
        }),
    )
    .into_any_element()
}

fn color_control(
    pane: &SettingsView,
    id: &'static str,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let color = color(&pane.snapshot.terminal, id);
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(swatch(pane, color))
        .child(pane.field(id))
        .child(change_button(
            pane,
            &format!("{id}-theme"),
            &tr!("Use Theme"),
            color.is_some(),
            Change::Terminal(id, String::new()),
            cx,
        ))
        .into_any_element()
}

fn palette_control(pane: &SettingsView) -> AnyElement {
    let palette = &pane.snapshot.terminal.options.palette;
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.0))
                .children(PALETTE.into_iter().zip(0_u8..).map(|(id, index)| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .w(px(18.0))
                                .text_size(px(12.0))
                                .text_color(pane.theme.fg_muted)
                                .child(index.to_string()),
                        )
                        .child(swatch(
                            pane,
                            palette.get(&index).copied().map(TerminalColor::Rgb),
                        ))
                        .child(controls::text_field(
                            pane.style(),
                            id,
                            &pane.fields[id],
                            Some(84.0),
                        ))
                })),
        )
        .children(
            PALETTE
                .into_iter()
                .filter_map(|id| pane.errors.get(id).map(|error| pane.note(error, true))),
        )
        .into_any_element()
}

fn style_font(pane: &SettingsView, slot: FontSlot, cx: &mut Context<SettingsView>) -> AnyElement {
    let font = &pane.snapshot.terminal.font;
    let current = match slot {
        FontSlot::Bold => font.bold.first(),
        FontSlot::Italic => font.italic.first(),
        _ => font.bold_italic.first(),
    };
    let default = tr!("Default");
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .child(pane.picker(
            PickerKind::Font(slot),
            current.map_or(default.as_ref(), String::as_str),
            cx,
        ))
        .when(current.is_some(), |row| {
            row.child(change_button(
                pane,
                &format!("{}-default", slot.id()),
                &tr!("Use Default"),
                true,
                Change::Terminal(slot.id(), String::new()),
                cx,
            ))
        })
        .into_any_element()
}

#[derive(Clone, Copy)]
pub(super) struct Slider {
    id: &'static str,
    min: f32,
    max: f32,
    step: f32,
    suffix: &'static str,
}

pub(super) struct Drag {
    slider: Slider,
    bounds: gpui::Bounds<gpui::Pixels>,
    value: Option<String>,
}

impl Drag {
    pub(super) fn change(self) -> Option<Change> {
        self.value
            .map(|value| Change::Terminal(self.slider.id, value))
    }
}

fn adjustment(terminal: &TerminalSettings, id: &str) -> CellHeight {
    match id {
        "adjust-cell-height" => terminal.cell_height,
        "adjust-cell-width" => terminal.font.cell_width,
        _ => terminal.options.cursor_thickness,
    }
}

pub(super) fn choices(pane: &SettingsView, id: &str) -> (String, Vec<(&'static str, String)>) {
    let terminal = &pane.snapshot.terminal;
    let (selected, items) = match id {
        "cursor-style" => (
            match terminal.options.cursor_style {
                None => "",
                Some(muxy_protocol::CursorShape::Bar) => "bar",
                Some(muxy_protocol::CursorShape::Underline) => "underline",
                Some(muxy_protocol::CursorShape::Hollow) => "block_hollow",
                Some(_) => "block",
            }
            .to_owned(),
            vec![
                ("", tr!("Default")),
                ("block", tr!("Block")),
                ("bar", tr!("Bar")),
                ("underline", tr!("Underline")),
                ("block_hollow", tr!("Outline")),
            ],
        ),
        "cursor-style-blink" => (
            match terminal.options.cursor_blink {
                None => "",
                Some(true) => "true",
                Some(false) => "false",
            }
            .to_owned(),
            vec![
                ("", tr!("Default")),
                ("true", tr!("On")),
                ("false", tr!("Off")),
            ],
        ),
        "macos-option-as-alt" => (
            terminal.macos_option_as_alt.to_string(),
            vec![
                ("true", tr!("Both")),
                ("false", tr!("Neither")),
                ("left", tr!("Left")),
                ("right", tr!("Right")),
            ],
        ),
        "directory" => (
            (if pane.snapshot.settings.panes.new_pane_directory == NewPaneDirectory::Current {
                "current"
            } else {
                "project"
            })
            .to_owned(),
            vec![
                ("project", tr!("Project")),
                ("current", tr!("Current pane")),
            ],
        ),
        "window-padding-color" => (
            match terminal.options.padding_color {
                PaddingColor::Background => "background",
                PaddingColor::Extend => "extend",
                PaddingColor::ExtendAlways => "extend-always",
            }
            .to_owned(),
            vec![
                ("background", tr!("Background")),
                ("extend", tr!("Extend")),
                ("extend-always", tr!("Extend always")),
            ],
        ),
        BINDING_ACTION => return terminal_lists::action_choices(pane),
        _ => (
            (if matches!(adjustment(terminal, id), CellHeight::Percent(_)) {
                "percent"
            } else {
                "pixels"
            })
            .to_owned(),
            vec![("pixels", tr!("Pixels")), ("percent", tr!("Percent"))],
        ),
    };
    (
        selected,
        items
            .into_iter()
            .map(|(id, label)| (id, label.to_string()))
            .collect(),
    )
}

pub(super) fn choice_change(pane: &SettingsView, id: &'static str, value: &str) -> Change {
    if id == "directory" {
        return Change::Directory(if value == "current" {
            NewPaneDirectory::Current
        } else {
            NewPaneDirectory::Project
        });
    }
    if id.starts_with("adjust-") {
        let amount = match adjustment(&pane.snapshot.terminal, id) {
            CellHeight::Natural => 0.0,
            CellHeight::Pixels(value) => f32::from(value),
            CellHeight::Percent(value) => value,
        };
        return Change::Terminal(
            id,
            if value == "percent" {
                format!("{}%", amount.clamp(PERCENT_RANGE.0, PERCENT_RANGE.1))
            } else {
                amount
                    .round()
                    .clamp(PIXEL_RANGE.0, PIXEL_RANGE.1)
                    .to_string()
            },
        );
    }
    Change::Terminal(id, value.to_owned())
}

fn picker(pane: &SettingsView, id: &'static str, cx: &mut Context<SettingsView>) -> AnyElement {
    let (selected, choices) = choices(pane, id);
    let label = choices
        .iter()
        .find(|(id, _)| *id == selected)
        .map_or("", |(_, label)| label.as_str());
    pane.picker(PickerKind::Terminal(id), label, cx)
}

fn slider(
    pane: &SettingsView,
    id: &'static str,
    value: f32,
    range: (f32, f32),
    step: f32,
    suffix: &'static str,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let spec = Slider {
        id,
        min: range.0.min(value),
        max: range.1.max(value),
        step,
        suffix,
    };
    let control = controls::slider(
        pane.style(),
        id,
        value,
        (spec.min, spec.max),
        cx.listener(move |view, grab: &controls::Grab, _, cx| {
            view.finish_terminal_slider(cx);
            view.terminal_slider = Some(Drag {
                slider: spec,
                bounds: grab.bounds,
                value: None,
            });
            view.move_terminal_slider(grab.position, cx);
        }),
    );
    let display = if suffix == "%" {
        format!("{value:.0}%")
    } else if step >= 1.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    };
    div()
        .debug_selector(move || format!("terminal-slider-{id}"))
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(control)
        .child(div().min_w(px(44.0)).child(display))
        .into_any_element()
}

fn adjustment_slider(
    pane: &SettingsView,
    id: &'static str,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let (value, suffix) = match adjustment(&pane.snapshot.terminal, id) {
        CellHeight::Natural => (0.0, ""),
        CellHeight::Pixels(value) => (f32::from(value), ""),
        CellHeight::Percent(value) => (value, "%"),
    };
    let range = if suffix == "%" {
        PERCENT_RANGE
    } else {
        PIXEL_RANGE
    };
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .child(slider(pane, id, value, range, 1.0, suffix, cx))
        .child(picker(pane, id, cx))
        .into_any_element()
}

#[allow(clippy::too_many_lines)]
pub(super) fn rows(
    pane: &SettingsView,
    _: &Window,
    cx: &mut Context<SettingsView>,
) -> Vec<AnyElement> {
    let terminal = &pane.snapshot.terminal;
    let options = &terminal.options;
    let mut rows = Vec::new();
    for setting in SECTIONS
        .iter()
        .flat_map(|section| {
            catalog::SETTINGS.iter().filter(move |setting| {
                setting.category == Category::Terminal && setting.section == *section
            })
        })
        .filter(|setting| pane.matches(Category::Terminal, setting.label))
    {
        let id = setting.id;
        let toggle = match id {
            "font-ligatures" => Some(terminal.ligatures_enabled()),
            "font-thicken" => Some(terminal.font.thicken),
            "bold-is-bright" => Some(options.bold_is_bright),
            "background-opacity-cells" => Some(options.background_opacity_cells),
            "window-padding-balance" => Some(options.padding_balance),
            "copy-on-select" => Some(
                options
                    .copy_on_select
                    .unwrap_or(pane.snapshot.settings.clipboard.copy_on_select),
            ),
            "selection-clear-on-typing" => Some(options.selection_clear_on_typing),
            "selection-clear-on-copy" => Some(options.selection_clear_on_copy),
            "scroll-on-keystroke" => Some(options.scroll_on_keystroke),
            "scroll-on-output" => Some(options.scroll_on_output),
            "mouse-reporting" => Some(options.mouse_reporting),
            "keybind-clear-defaults" => Some(terminal.keybindings.clear_defaults),
            _ => None,
        };
        let control = if let Some(value) = toggle {
            pane.toggle(id, value, Change::Terminal(id, (!value).to_string()), cx)
        } else {
            match id {
                "font-family" => pane.picker(
                    PickerKind::Font(FontSlot::Regular),
                    terminal
                        .font_families
                        .first()
                        .map_or("Menlo", String::as_str),
                    cx,
                ),
                "font-size" => slider(pane, id, terminal.font_size, (6.0, 48.0), 0.5, "", cx),
                "font-family-bold" => style_font(pane, FontSlot::Bold, cx),
                "font-family-italic" => style_font(pane, FontSlot::Italic, cx),
                "font-family-bold-italic" => style_font(pane, FontSlot::BoldItalic, cx),
                "font-feature" => pane.field(id),
                "font-thicken-strength" => slider(
                    pane,
                    id,
                    f32::from(terminal.font.thicken_strength),
                    (0.0, 255.0),
                    1.0,
                    "",
                    cx,
                ),
                "cursor-opacity" => slider(
                    pane,
                    id,
                    options.cursor_opacity * 100.0,
                    (0.0, 100.0),
                    1.0,
                    "%",
                    cx,
                ),
                "background"
                | "foreground"
                | "cursor-color"
                | "cursor-text"
                | "selection-foreground"
                | "selection-background" => color_control(pane, id, cx),
                "font-fallbacks" | "font-codepoint-map" | "palette" | "terminal-keybindings" => {
                    let control = match id {
                        "font-fallbacks" => terminal_lists::fallbacks(pane, cx),
                        "font-codepoint-map" => terminal_lists::codepoint_maps(pane, cx),
                        "palette" => palette_control(pane),
                        _ => terminal_lists::bindings(pane, cx),
                    };
                    rows.push(pane.row_with(id, setting.label, control, true));
                    continue;
                }
                "background-transparency" => slider(
                    pane,
                    id,
                    (1.0 - options.background_opacity.unwrap_or(1.0)) * 100.0,
                    (0.0, 100.0),
                    1.0,
                    "%",
                    cx,
                ),
                "background-vibrancy" => slider(
                    pane,
                    id,
                    f32::from(options.background_vibrancy),
                    (0.0, 100.0),
                    1.0,
                    "%",
                    cx,
                ),
                "adjust-cell-height" | "adjust-cell-width" | "adjust-cursor-thickness" => {
                    adjustment_slider(pane, id, cx)
                }
                "scroll-precision" | "scroll-discrete" => slider(
                    pane,
                    id,
                    if id == "scroll-precision" {
                        options.scroll_precision
                    } else {
                        options.scroll_discrete
                    },
                    (0.01, 10.0),
                    0.01,
                    "",
                    cx,
                ),
                "window-padding-x" | "window-padding-y" => {
                    let items = if id == "window-padding-x" {
                        [
                            ("padding-left", tr!("Left"), options.padding_x[0]),
                            ("padding-right", tr!("Right"), options.padding_x[1]),
                        ]
                    } else {
                        [
                            ("padding-top", tr!("Top"), options.padding_y[0]),
                            ("padding-bottom", tr!("Bottom"), options.padding_y[1]),
                        ]
                    };
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .children(items.into_iter().map(|(id, label, value)| {
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(div().w(px(60.0)).child(label))
                                .child(slider(pane, id, value, (0.0, 64.0), 1.0, "", cx))
                        }))
                        .into_any_element()
                }
                "cursor-style"
                | "cursor-style-blink"
                | "macos-option-as-alt"
                | "directory"
                | "window-padding-color" => picker(pane, id, cx),
                "terminal-import-notes" if !terminal.diagnostics.is_empty() => {
                    let messages = terminal.diagnostics.iter().map(|note| {
                        note.split_once(": ")
                            .map_or(note.as_str(), |(_, text)| text)
                    });
                    let notes = div()
                        .w_full()
                        .min_w(px(0.0))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .children(messages.map(|text| div().min_w(px(0.0)).child(text.to_owned())))
                        .into_any_element();
                    let control = div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(notes)
                        .child(div().flex().child(change_button(
                            pane,
                            "dismiss-import-notes",
                            &tr!("Dismiss"),
                            true,
                            Change::Terminal("dismiss-import-notes", String::new()),
                            cx,
                        )))
                        .into_any_element();
                    rows.push(pane.row_with_description(id, setting.label, None, control, true));
                    continue;
                }
                "terminal-reset" => controls::button(
                    pane.style(),
                    id,
                    &tr!("Reset"),
                    *terminal != TerminalSettings::default()
                        || pane.snapshot.settings.panes.new_pane_directory
                            != NewPaneDirectory::default(),
                    cx.listener(|pane, _, _, cx| {
                        pane.recording = None;
                        cx.emit(SettingsEvent::ResetTerminal);
                    }),
                )
                .debug_selector(|| "settings-button-terminal-reset".into())
                .into_any_element(),
                _ => continue,
            }
        };
        rows.push(pane.row(id, setting.label, control));
    }
    rows
}

impl SettingsView {
    pub(super) fn active_font(&self, slot: FontSlot) -> String {
        let terminal = &self.snapshot.terminal;
        match slot {
            FontSlot::Regular => terminal.font_families.first(),
            FontSlot::Fallback => None,
            FontSlot::Bold => terminal.font.bold.first(),
            FontSlot::Italic => terminal.font.italic.first(),
            FontSlot::BoldItalic => terminal.font.bold_italic.first(),
            FontSlot::Codepoint => self
                .codepoint_editor
                .as_ref()
                .map(terminal_lists::CodepointEditor::family),
        }
        .cloned()
        .unwrap_or_default()
    }

    pub(super) fn select_font(&mut self, slot: FontSlot, name: String, cx: &mut Context<Self>) {
        match slot {
            FontSlot::Fallback => self.add_fallback(name, cx),
            FontSlot::Codepoint => self.choose_codepoint_font(name, cx),
            slot => cx.emit(SettingsEvent::Change(Change::Terminal(slot.id(), name))),
        }
    }

    pub(super) fn choose_terminal_option(
        &mut self,
        id: &'static str,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        if id == BINDING_ACTION {
            self.choose_binding_action(value, cx);
        } else {
            cx.emit(SettingsEvent::Change(choice_change(self, id, value)));
        }
    }

    pub(super) fn move_terminal_slider(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = &mut self.terminal_slider else {
            return;
        };
        let spec = drag.slider;
        let fraction = controls::fraction_at(drag.bounds, position);
        let value = ((spec.min + fraction * (spec.max - spec.min)) / spec.step).round() * spec.step;
        let value = value.clamp(spec.min, spec.max);
        let value = format!(
            "{}{}",
            if spec.step >= 1.0 {
                format!("{value:.0}")
            } else {
                format!("{value:.2}")
            },
            if spec.id.starts_with("adjust-") {
                spec.suffix
            } else {
                ""
            }
        );
        if drag.value.as_ref() != Some(&value) {
            drag.value = Some(value.clone());
            cx.emit(SettingsEvent::PreviewTerminal(spec.id, value));
        }
    }

    pub(super) fn finish_terminal_slider(&mut self, cx: &mut Context<Self>) {
        if let Some(change) = self.terminal_slider.take().and_then(Drag::change) {
            cx.emit(SettingsEvent::Change(change));
        }
    }
}
