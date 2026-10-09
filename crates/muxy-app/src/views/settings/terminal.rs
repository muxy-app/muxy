use super::{Category, Change, PickerKind, SettingsEvent, SettingsView, catalog};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div, px,
};
use muxy_app_core::settings::{CellHeight, NewPaneDirectory, TerminalSettings};
use muxy_ui::controls;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

pub(super) const SECTIONS: &[&str] = &[
    tr_key!("Font"),
    tr_key!("Background"),
    tr_key!("Cursor"),
    tr_key!("Selection"),
    tr_key!("Scrolling"),
    tr_key!("Input"),
    tr_key!("Migration"),
];
pub(super) const PICKERS: &[&str] = &[
    "cursor-style",
    "cursor-style-blink",
    "macos-option-as-alt",
    "directory",
    "adjust-cell-height",
    "adjust-cell-width",
    "adjust-cursor-thickness",
];

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
                format!("{}%", amount.clamp(-99.0, 1000.0))
            } else {
                amount.round().to_string()
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
        (-50.0, 200.0)
    } else {
        (-5.0, 20.0)
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
            _ => None,
        };
        let control = if let Some(value) = toggle {
            pane.toggle(id, value, Change::Terminal(id, (!value).to_string()), cx)
        } else {
            match id {
                "font-family" => pane.picker(
                    PickerKind::FontFamily,
                    terminal
                        .font_families
                        .first()
                        .map_or("Menlo", String::as_str),
                    cx,
                ),
                "font-size" => slider(pane, id, terminal.font_size, (6.0, 48.0), 0.5, "", cx),
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
                "cursor-style" | "cursor-style-blink" | "macos-option-as-alt" | "directory" => {
                    picker(pane, id, cx)
                }
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
                    rows.push(pane.row_with_description(id, setting.label, None, notes, true));
                    continue;
                }
                _ => continue,
            }
        };
        rows.push(pane.row(id, setting.label, control));
    }
    rows
}

impl SettingsView {
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
