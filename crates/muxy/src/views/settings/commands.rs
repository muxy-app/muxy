use super::{Field, SettingsModal};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_core::fold::fold;
use muxy_ui::components::SymbolGlyph;
use muxy_ui::controls::{self, Style};

const SEARCH: &str = "command.search";
const EXPLANATION: &str =
    "Press the command layer shortcut, then a command key to open a new terminal tab.";

pub fn fields(modal: &SettingsModal) -> Vec<Field> {
    let mut specs = vec![Field {
        id: SEARCH.to_owned(),
        value: modal.command_query().to_owned(),
        placeholder: "Search commands".to_owned(),
        monospaced: false,
        multiline: false,
    }];
    for shortcut in &modal.commands().shortcuts {
        specs.push(Field {
            id: format!("command.name.{}", shortcut.id),
            value: shortcut.name.clone(),
            placeholder: "Name".to_owned(),
            monospaced: false,
            multiline: false,
        });
        specs.push(Field {
            id: format!("command.line.{}", shortcut.id),
            value: shortcut.command.clone(),
            placeholder: "Command".to_owned(),
            monospaced: true,
            multiline: false,
        });
    }
    specs
}

pub fn commit_field(
    modal: &mut SettingsModal,
    id: &str,
    text: &str,
    cx: &mut Context<SettingsModal>,
) -> bool {
    if id == SEARCH {
        return true;
    }
    if let Some(command_id) = id.strip_prefix("command.name.") {
        let command_id = command_id.to_owned();
        modal.update_command(&command_id, Some(text.to_owned()), None, cx);
        return true;
    }
    if let Some(command_id) = id.strip_prefix("command.line.") {
        let command_id = command_id.to_owned();
        modal.update_command(&command_id, None, Some(text.to_owned()), cx);
        return true;
    }
    false
}

pub fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let style = modal.style();
    let metrics = style.metrics;
    let query = fold(modal.command_query().trim());

    let mut block = div()
        .flex()
        .flex_col()
        .gap(metrics.spacing4())
        .p(metrics.spacing6())
        .child(header(modal, cx))
        .child(
            div()
                .text_size(metrics.font_footnote())
                .text_color(style.theme.fg_muted)
                .child(SharedString::from(EXPLANATION)),
        )
        .child(prefix_row(modal, cx));

    if let Some(message) = modal.command_conflict() {
        block = block.child(
            div()
                .text_size(metrics.font_footnote())
                .text_color(style.theme.danger)
                .child(SharedString::from(message.to_owned())),
        );
    }

    let ids: Vec<String> = modal
        .commands()
        .shortcuts
        .iter()
        .filter(|shortcut| {
            query.is_empty()
                || fold(&shortcut.name).contains(&query)
                || fold(&shortcut.command).contains(&query)
        })
        .map(|shortcut| shortcut.id.clone())
        .collect();

    if ids.is_empty() {
        block = block.child(
            div()
                .py(metrics.spacing5())
                .text_size(metrics.font_footnote())
                .text_color(style.theme.fg_muted)
                .child(SharedString::from("No commands yet.")),
        );
    }
    for id in ids {
        block = block.child(command_row(modal, &id, cx));
    }

    block = block.child(delete_all(modal, cx));
    vec![block.into_any_element()]
}

fn header(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> AnyElement {
    let style = modal.style();
    let metrics = style.metrics;
    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing4());

    if let Some(field) = modal.field(SEARCH) {
        bar = bar.child(controls::text_field(style, SEARCH, field, None));
    }

    bar.child(
        div()
            .id("command-add")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(metrics.control_medium())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .bg(style.theme.surface)
            .border_1()
            .border_color(style.theme.border)
            .hover(|hover| hover.bg(style.theme.hover))
            .child(SymbolGlyph::new(
                "plus",
                metrics.font_footnote(),
                style.theme.fg,
            ))
            .on_click(
                cx.listener(|modal: &mut SettingsModal, _, window: &mut Window, cx| {
                    modal.add_command(window, cx);
                }),
            ),
    )
    .into_any_element()
}

fn prefix_row(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> AnyElement {
    let style = modal.style();
    let armed = modal.armed_command() == Some("prefix");
    let label = if armed {
        "Press a shortcut…".to_owned()
    } else {
        modal.commands().prefix_combo.display()
    };
    controls::row(
        style,
        "Command Layer",
        chip(style, "prefix", &label, armed, cx),
    )
}

fn chip(
    style: Style,
    target: &str,
    label: &str,
    armed: bool,
    cx: &mut Context<SettingsModal>,
) -> AnyElement {
    let metrics = style.metrics;
    let owned = target.to_owned();
    div()
        .id(SharedString::from(format!("command-chip-{target}")))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(metrics.control_medium())
        .px(metrics.spacing4())
        .rounded(metrics.radius_sm())
        .cursor_pointer()
        .bg(style.theme.surface)
        .border_1()
        .border_color(if armed {
            style.theme.accent
        } else {
            style.theme.border
        })
        .text_size(metrics.font_footnote())
        .text_color(style.theme.fg)
        .child(SharedString::from(label.to_owned()))
        .on_click(cx.listener(
            move |modal: &mut SettingsModal, _, window: &mut Window, cx| {
                modal.arm_command(&owned, window, cx);
            },
        ))
        .into_any_element()
}

fn command_row(modal: &SettingsModal, id: &str, cx: &mut Context<SettingsModal>) -> AnyElement {
    let style = modal.style();
    let metrics = style.metrics;
    let armed = modal.armed_command() == Some(id);
    let prefix = modal.commands().prefix_combo.display();
    let combo = modal
        .commands()
        .shortcut(id)
        .map(|shortcut| shortcut.combo.display())
        .unwrap_or_default();
    let label = if armed {
        "Press a shortcut…".to_owned()
    } else {
        format!("{prefix} {combo}").trim().to_owned()
    };
    let owned = id.to_owned();

    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing4());

    if let Some(field) = modal.field(&format!("command.name.{id}")) {
        row = row.child(controls::text_field(
            style,
            &format!("name-{id}"),
            field,
            Some(120.0),
        ));
    }
    if let Some(field) = modal.field(&format!("command.line.{id}")) {
        row = row.child(controls::text_field(
            style,
            &format!("line-{id}"),
            field,
            None,
        ));
    }

    row.child(chip(style, id, &label, armed, cx))
        .child(
            div()
                .id(SharedString::from(format!("command-delete-{id}")))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(metrics.control_medium())
                .rounded(metrics.radius_sm())
                .cursor_pointer()
                .hover(|hover| hover.bg(style.theme.hover))
                .child(SymbolGlyph::new(
                    "trash",
                    metrics.font_footnote(),
                    style.theme.fg_muted,
                ))
                .on_click(cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
                    modal.remove_command(&owned, cx);
                })),
        )
        .into_any_element()
}

fn delete_all(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> AnyElement {
    let style = modal.style();
    let metrics = style.metrics;
    let countdown = modal.delete_all_countdown();
    let label = match countdown {
        Some(remaining) => format!("Confirm Delete All ({remaining})"),
        None => "Delete All".to_owned(),
    };

    div()
        .flex()
        .flex_row()
        .justify_end()
        .pt(metrics.spacing4())
        .child(
            div()
                .id("command-delete-all")
                .flex()
                .flex_none()
                .items_center()
                .h(metrics.control_medium())
                .px(metrics.spacing4())
                .rounded(metrics.radius_sm())
                .cursor_pointer()
                .hover(|hover| hover.bg(style.theme.hover))
                .text_size(metrics.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.theme.danger)
                .child(SharedString::from(label))
                .on_click(cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
                    if modal.delete_all_countdown().is_some() {
                        modal.remove_all_commands(cx);
                    } else {
                        modal.start_delete_all(cx);
                    }
                })),
        )
        .into_any_element()
}

pub fn shortcuts(modal: &SettingsModal) -> Vec<AnyElement> {
    let Some(editor) = modal.editor() else {
        return Vec::new();
    };
    vec![
        div()
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .child(editor.clone())
            .into_any_element(),
    ]
}
