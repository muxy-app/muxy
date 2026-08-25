use super::*;

pub(super) fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let category = Category::Notifications;
    let style = modal.style();
    let mut sections = Vec::new();

    sections.extend(visible(
        modal,
        category,
        "Delivery",
        None,
        true,
        vec![
            toggle_row(style, "Toast", "muxy.notifications.toastEnabled", true, cx),
            toggle_row(
                style,
                "Desktop notifications",
                "muxy.notifications.desktopEnabled",
                false,
                cx,
            ),
        ],
    ));

    sections.extend(visible(
        modal,
        category,
        "Sound",
        None,
        true,
        vec![picker_row(
            modal,
            "Sound",
            "muxy.notifications.sound",
            "Funk",
            settings::NOTIFICATION_SOUNDS
                .iter()
                .map(|name| Choice::new(*name, *name))
                .collect(),
            cx,
        )],
    ));

    sections.extend(visible(
        modal,
        category,
        "Toast",
        None,
        true,
        vec![picker_row(
            modal,
            "Position",
            "muxy.notifications.toastPosition",
            "Top Center",
            settings::TOAST_POSITIONS
                .iter()
                .map(|name| Choice::new(*name, *name))
                .collect(),
            cx,
        )],
    ));

    let providers: Vec<AnyElement> = settings::AI_PROVIDERS
        .iter()
        .map(|(id, name)| provider_row(modal, id, name, cx))
        .collect();
    sections.extend(visible(
        modal,
        category,
        "AI Providers",
        None,
        false,
        providers,
    ));

    sections
}

fn provider_row(
    modal: &SettingsModal,
    id: &'static str,
    name: &'static str,
    cx: &mut Context<SettingsModal>,
) -> AnyElement {
    let style = modal.style();
    let key = settings::provider_key(id);
    let value = settings::bool_value(&key, true);
    controls::row(
        style,
        name,
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(style.metrics.spacing3())
            .child(controls::button(
                style,
                &format!("provider-test-{id}"),
                "Test",
                false,
                |_, _, _| {},
            ))
            .child(controls::button(
                style,
                &format!("provider-refresh-{id}"),
                "Refresh",
                false,
                |_, _, _| {},
            ))
            .child(controls::toggle(
                style,
                &key,
                value,
                cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
                    let key = settings::provider_key(id);
                    let next = !settings::bool_value(&key, true);
                    modal.write_ai_notification_provider(id, next, cx);
                }),
            ))
            .into_any_element(),
    )
}
