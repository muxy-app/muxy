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
            desktop_notifications_row(modal, cx),
        ],
    ));

    sections.extend(visible(
        modal,
        category,
        "Sound",
        None,
        true,
        vec![notification_sound_row(modal, cx)],
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

    let providers: Vec<AnyElement> = muxy_core::repository_ai::PROVIDERS
        .iter()
        .map(|provider| provider_row(modal, provider.id, provider.display_name, cx))
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

fn desktop_notifications_row(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> AnyElement {
    let style = modal.style();
    let value = modal.desktop_authorization_pending()
        || settings::bool_value("muxy.notifications.desktopEnabled", false);
    controls::row(
        style,
        "Desktop notifications",
        controls::toggle(
            style,
            "muxy.notifications.desktopEnabled",
            value,
            cx.listener(|modal: &mut SettingsModal, _, _, cx| {
                modal.request_desktop_notifications(cx);
            }),
        ),
    )
}

fn notification_sound_row(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> AnyElement {
    let key = "muxy.notifications.sound";
    let choices = settings::NOTIFICATION_SOUNDS
        .iter()
        .map(|name| Choice::new(*name, *name))
        .collect::<Vec<_>>();
    let selected = settings::string_value(key, "Funk");
    let popover = modal.picker(key).cloned();
    let toggle_choices = choices.clone();
    let toggle_selected = selected.clone();
    controls::row(
        modal.style(),
        "Sound",
        controls::picker(
            modal.style(),
            key,
            choices,
            &selected,
            popover,
            cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
                modal.toggle_picker(
                    key,
                    toggle_choices.clone(),
                    toggle_selected.clone(),
                    SettingsPickerTarget::NotificationSound,
                    cx,
                );
            }),
        ),
    )
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
