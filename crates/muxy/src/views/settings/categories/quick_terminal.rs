use super::*;

pub(super) fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let category = Category::QuickTerminal;
    let style = modal.style();
    let metrics = style.metrics;
    let enabled = settings::bool_value("muxy.quickTerminal.enabled", true);
    let mut sections = Vec::new();

    sections.extend(visible(
        modal,
        category,
        "General",
        (!enabled).then_some(
            "The Quick Terminal shortcut listener and shell are off. Your shortcut, size, and appearance settings are preserved.",
        ),
        true,
        vec![toggle_row(
            style,
            "Enable Quick Terminal",
            "muxy.quickTerminal.enabled",
            true,
            cx,
        )],
    ));

    let kind = settings::quick_terminal_kind();
    let status = if !enabled {
        "Disabled"
    } else if kind == "doubleShift" {
        "Active system-wide"
    } else {
        "No shortcut assigned"
    };
    sections.extend(visible(
        modal,
        category,
        "Shortcut",
        None,
        true,
        vec![
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(metrics.spacing4())
                .px(metrics.spacing6())
                .py(metrics.spacing3())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(metrics.spacing1())
                        .flex_grow()
                        .child(
                            div()
                                .text_size(metrics.font_body())
                                .text_color(style.theme.fg)
                                .child(SharedString::from("Open Quick Terminal")),
                        )
                        .child(
                            div()
                                .text_size(metrics.font_footnote())
                                .text_color(style.theme.fg_muted)
                                .child(SharedString::from(status)),
                        ),
                )
                .child(shortcut_option(
                    modal,
                    "unassigned",
                    "No Shortcut",
                    &kind,
                    cx,
                ))
                .child(shortcut_option(
                    modal,
                    "doubleShift",
                    "Double Shift",
                    &kind,
                    cx,
                ))
                .child(controls::button(
                    style,
                    "quick-record",
                    "Record Custom…",
                    false,
                    |_, _, _| {},
                ))
                .into_any_element(),
        ],
    ));

    let mut size_row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing4())
        .px(metrics.spacing6())
        .py(metrics.spacing3())
        .child(
            div()
                .flex_grow()
                .text_size(metrics.font_body())
                .text_color(style.theme.fg)
                .child(SharedString::from("Terminal size")),
        );
    for (key, label) in [(QUICK_WIDTH, "Width"), (QUICK_HEIGHT, "Height")] {
        size_row = size_row.child(
            div()
                .flex_none()
                .text_size(metrics.font_footnote())
                .text_color(style.theme.fg_muted)
                .child(SharedString::from(label)),
        );
        if let Some(field) = modal.field(key) {
            size_row = size_row.child(controls::text_field(style, key, field, Some(80.0)));
        }
    }
    size_row = size_row.child(controls::button(
        style,
        "quick-size-reset",
        "Reset",
        true,
        cx.listener(|modal: &mut SettingsModal, _, _, cx| {
            modal.write(QUICK_WIDTH, Value::Number(720.into()), cx);
            modal.write(QUICK_HEIGHT, Value::Number(430.into()), cx);
            modal.reset_field(QUICK_WIDTH, "720", cx);
            modal.reset_field(QUICK_HEIGHT, "430", cx);
        }),
    ));
    sections.extend(visible(
        modal,
        category,
        "Size",
        None,
        true,
        vec![size_row.into_any_element()],
    ));

    let transparency = modal.slider_value(
        QUICK_TRANSPARENCY_SLIDER,
        settings::i64_value(QUICK_TRANSPARENCY, 18) as f32,
    );
    let blur = modal.slider_value(
        QUICK_BLUR_SLIDER,
        settings::i64_value(QUICK_BLUR, 70) as f32,
    );
    sections.extend(visible(
        modal,
        category,
        "Appearance",
        None,
        false,
        vec![
            div()
                .flex()
                .flex_col()
                .gap(metrics.spacing4())
                .px(metrics.spacing6())
                .py(metrics.spacing3())
                .child(
                    appearance::slider_line(
                        modal,
                        "Terminal transparency",
                        QUICK_TRANSPARENCY_SLIDER,
                        transparency,
                        format!("{}%", transparency as i64),
                        cx,
                    )
                    .child(div().w(metrics.scaled(64.0)).flex_none()),
                )
                .child(
                    appearance::slider_line(
                        modal,
                        "Background vibrancy",
                        QUICK_BLUR_SLIDER,
                        blur,
                        format!("{}%", blur as i64),
                        cx,
                    )
                    .child(div().flex_none().child(controls::button(
                        style,
                        "quick-appearance-reset",
                        "Reset",
                        true,
                        cx.listener(|modal: &mut SettingsModal, _, _, cx| {
                            modal.write(QUICK_TRANSPARENCY, Value::Number(18.into()), cx);
                            modal.write(QUICK_BLUR, Value::Number(70.into()), cx);
                        }),
                    ))),
                )
                .into_any_element(),
        ],
    ));

    sections
}

fn shortcut_option(
    modal: &SettingsModal,
    kind: &'static str,
    label: &'static str,
    current: &str,
    cx: &mut Context<SettingsModal>,
) -> AnyElement {
    let style = modal.style();
    let selected = current == kind;
    div()
        .id(SharedString::from(format!("quick-shortcut-{kind}")))
        .flex()
        .flex_none()
        .flex_row()
        .items_center()
        .gap(style.metrics.spacing3())
        .h(style.metrics.control_medium())
        .px(style.metrics.spacing4())
        .rounded(style.metrics.radius_sm())
        .cursor_pointer()
        .bg(style.theme.surface)
        .border_1()
        .border_color(if selected {
            style.theme.accent
        } else {
            style.theme.border
        })
        .hover(|hover| hover.bg(style.theme.hover))
        .text_size(style.metrics.font_footnote())
        .text_color(style.theme.fg)
        .child(SymbolGlyph::new(
            if selected {
                "checkmark.circle.fill"
            } else {
                "circle"
            },
            style.metrics.font_footnote(),
            if selected {
                style.theme.accent
            } else {
                style.theme.fg_muted
            },
        ))
        .child(SharedString::from(label))
        .on_click(cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
            settings::set_quick_terminal_shortcut(kind);
            modal.refresh(cx);
        }))
        .into_any_element()
}
