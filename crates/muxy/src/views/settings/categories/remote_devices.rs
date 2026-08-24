use super::*;

pub(super) fn content(modal: &SettingsModal) -> Vec<AnyElement> {
    let style = modal.style();
    let metrics = style.metrics;
    let devices = muxy_core::store::remote_devices();
    let mut items: Vec<AnyElement> = if devices.is_empty() {
        vec![
            div()
                .px(metrics.spacing6())
                .py(metrics.spacing3())
                .text_size(metrics.font_body())
                .text_color(style.theme.fg_muted)
                .child(SharedString::from("No remote devices yet."))
                .into_any_element(),
        ]
    } else {
        devices
            .into_iter()
            .map(|name| {
                controls::row(
                    style,
                    &name,
                    browser::inert_actions(style, &name, &["Edit", "Delete"]),
                )
            })
            .collect()
    };
    items.push(controls::row(
        style,
        "",
        controls::button(
            style,
            "remote-add",
            "Add Remote Device",
            false,
            |_, _, _| {},
        ),
    ));

    visible(
        modal,
        Category::RemoteDevices,
        "Remote Devices",
        Some("Remote devices are reusable SSH connections. Workspaces connect through a device, so you can reuse the same server without re-entering its details."),
        false,
        items,
    )
    .into_iter()
    .collect()
}
