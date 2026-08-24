use super::*;

pub(super) fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let style = modal.style();
    let multiplier = settings::editor_setting("richInputLineHeightMultiplier", Value::from(1.2))
        .as_f64()
        .unwrap_or(1.2);

    let mut items = vec![
        segmented_row(
            modal,
            "Presentation",
            "muxy.richInput.presentationMode",
            "panel",
            vec![
                Choice::new("panel", "Panel"),
                Choice::new("floating", "Floating"),
            ],
            cx,
        ),
        editor_picker_row(
            modal,
            "Image Submission",
            COMPOSER_IMAGE,
            "clipboard",
            vec![
                Choice::new("clipboard", "Clipboard Paste"),
                Choice::new("inlinePath", "Inline File Path"),
            ],
            cx,
        ),
    ];
    if let Some(field) = modal.field(COMPOSER_FONT) {
        items.push(controls::row(
            style,
            "Font Family",
            controls::text_field(style, COMPOSER_FONT, field, Some(controls::CONTROL_WIDTH)),
        ));
    }
    items.push(line_height_row(modal, multiplier, cx));
    items.push(toggle_row(
        style,
        "Clear After Sending",
        "muxy.richInput.clearAfterSending",
        false,
        cx,
    ));
    items.push(toggle_row(
        style,
        "Clear on Close",
        "muxy.richInput.clearOnClose",
        false,
        cx,
    ));

    visible(
        modal,
        Category::RichInput,
        "Composer",
        Some("Inline File Path keeps multiple images perfectly ordered with text and Enter. Use Clipboard Paste if your TUI doesn't recognize image paths. SSH panes always upload the image and inline its remote path, because a Mac file path does not resolve on the remote device."),
        false,
        items,
    )
    .into_iter()
    .collect()
}

fn editor_picker_row(
    modal: &SettingsModal,
    label: &str,
    key: &'static str,
    default: &'static str,
    choices: Vec<Choice>,
    cx: &mut Context<SettingsModal>,
) -> AnyElement {
    let style = modal.style();
    let name = key.strip_prefix("editor.").unwrap_or(key);
    let selected = editor_string(key, default);
    controls::row(
        style,
        label,
        controls::picker(
            style,
            key,
            choices,
            &selected,
            modal.open_picker() == Some(key),
            cx.listener(move |modal: &mut SettingsModal, _, _, cx| modal.toggle_picker(key, cx)),
            cx.listener(
                move |modal: &mut SettingsModal, value: &SharedString, _, cx| {
                    settings::set_editor_setting(name, Value::String(value.to_string()));
                    modal.close_picker(cx);
                    modal.refresh(cx);
                },
            ),
        ),
    )
}

fn line_height_row(
    modal: &SettingsModal,
    multiplier: f64,
    cx: &mut Context<SettingsModal>,
) -> AnyElement {
    let style = modal.style();
    let metrics = style.metrics;
    let step = |delta: f64| {
        move |modal: &mut SettingsModal,
              _: &gpui::ClickEvent,
              _: &mut gpui::Window,
              cx: &mut Context<SettingsModal>| {
            let current =
                settings::editor_setting("richInputLineHeightMultiplier", Value::from(1.2))
                    .as_f64()
                    .unwrap_or(1.2);
            let next = ((current + delta) * 10.0).round() / 10.0;
            let next = next.clamp(1.1, 2.0);
            settings::set_editor_setting(
                "richInputLineHeightMultiplier",
                serde_json::Number::from_f64(next).map_or(Value::Null, Value::Number),
            );
            modal.refresh(cx);
        }
    };

    controls::row(
        style,
        "Line Height",
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(metrics.spacing4())
            .child(controls::button(
                style,
                "line-height-down",
                "−",
                multiplier > 1.1 + 0.001,
                cx.listener(step(-0.1)),
            ))
            .child(
                div()
                    .w(metrics.scaled(44.0))
                    .text_size(metrics.font_body())
                    .text_color(style.theme.fg)
                    .child(SharedString::from(format!("{multiplier:.1}×"))),
            )
            .child(controls::button(
                style,
                "line-height-up",
                "+",
                multiplier < 2.0 - 0.001,
                cx.listener(step(0.1)),
            ))
            .into_any_element(),
    )
}
