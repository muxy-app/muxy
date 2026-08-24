use super::*;

pub(super) fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let category = Category::Mobile;
    let style = modal.style();
    let metrics = style.metrics;
    let mut sections = Vec::new();

    let mut items = vec![toggle_row(
        style,
        "Allow mobile device connections",
        "app.muxy.mobile.serverEnabled",
        false,
        cx,
    )];
    for (key, label) in [
        (MOBILE_PORT, "Port"),
        (MOBILE_CAP, "Scrollback per terminal (MB)"),
    ] {
        if let Some(field) = modal.field(key) {
            items.push(controls::row(
                style,
                label,
                controls::text_field(style, key, field, Some(controls::CONTROL_WIDTH)),
            ));
        }
        if let Some(error) = modal.error(key) {
            items.push(
                div()
                    .px(metrics.spacing6())
                    .pb(metrics.spacing3())
                    .text_size(metrics.font_footnote())
                    .text_color(style.theme.danger)
                    .child(SharedString::from(error.to_owned()))
                    .into_any_element(),
            );
        }
    }
    sections.extend(visible(
        modal,
        category,
        "Mobile",
        Some("Muxy listens on the configured port for the iOS app over your local network or a private VPN such as Tailscale."),
        true,
        items,
    ));

    sections.extend(visible(
        modal,
        category,
        "Pair Mobile Device",
        Some("Scan this with the Muxy mobile app to add this Mac. The QR carries no token — first-time pairing still needs your approval."),
        true,
        vec![
            div()
                .px(metrics.spacing6())
                .py(metrics.spacing3())
                .text_size(metrics.font_footnote())
                .text_color(style.theme.fg_muted)
                .child(SharedString::from(
                    "Pairing codes are shown by the Swift app.",
                ))
                .into_any_element(),
        ],
    ));

    let devices = muxy_core::store::approved_devices();
    let device_rows: Vec<AnyElement> = if devices.is_empty() {
        vec![
            div()
                .px(metrics.spacing6())
                .py(metrics.spacing3())
                .text_size(metrics.font_body())
                .text_color(style.theme.fg_muted)
                .child(SharedString::from("No devices approved yet."))
                .into_any_element(),
        ]
    } else {
        devices
            .into_iter()
            .map(|name| {
                controls::row(
                    style,
                    &name,
                    browser::inert_actions(style, &name, &["Revoke"]),
                )
            })
            .collect()
    };
    sections.extend(visible(
        modal,
        category,
        "Approved Devices",
        Some("Revoking removes the device's access. It will need to request approval again to reconnect."),
        false,
        device_rows,
    ));

    sections
}
