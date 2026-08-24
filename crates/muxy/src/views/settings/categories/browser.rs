use super::*;

pub(super) fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let category = Category::Browser;
    let style = modal.style();
    let enabled = settings::bool_value(BROWSER_ENABLED, true);
    let mut sections = Vec::new();

    let mut general = vec![toggle_row(
        style,
        "Enable Built-in Browser",
        BROWSER_ENABLED,
        true,
        cx,
    )];
    if enabled {
        general.push(toggle_row(
            style,
            "Open terminal links in built-in browser",
            BROWSER_LINKS,
            false,
            cx,
        ));
        let selected = settings::string_value(BROWSER_PROFILE, "");
        general.push(picker_row(
            modal,
            "Default Profile",
            BROWSER_PROFILE,
            "",
            appended_stored(
                muxy_core::store::browser_profiles()
                    .into_iter()
                    .map(|(id, name)| Choice::new(id, name))
                    .collect(),
                &selected,
                format!("{selected} (unavailable)"),
            ),
            cx,
        ));
    }
    sections.extend(visible(
        modal,
        category,
        "General",
        (!enabled).then_some(
            "The built-in browser is off. Browser tabs, the toolbar globe, and terminal-link opening are disabled, and terminal links open in your system browser.",
        ),
        enabled,
        general,
    ));

    if !enabled {
        return sections;
    }

    let uses_custom = settings::string_value(HOME_PAGE, BLANK_PAGE) != BLANK_PAGE;
    let mut browsing = vec![
        picker_row(
            modal,
            "Search Engine",
            BROWSER_ENGINE,
            "google",
            vec![
                Choice::new("google", "Google"),
                Choice::new("duckDuckGo", "DuckDuckGo"),
                Choice::new("bing", "Bing"),
                Choice::new("brave", "Brave"),
                Choice::new("startpage", "Startpage"),
            ],
            cx,
        ),
        controls::row(
            style,
            "Open new tabs to a website",
            controls::toggle(
                style,
                "browser-home-toggle",
                uses_custom,
                cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
                    let value = if uses_custom {
                        BLANK_PAGE.to_owned()
                    } else {
                        "https://example.com".to_owned()
                    };
                    modal.write(HOME_PAGE, Value::String(value), cx);
                }),
            ),
        ),
    ];
    if uses_custom && let Some(field) = modal.field(HOME_PAGE) {
        browsing.push(controls::row(
            style,
            "Home Page",
            controls::text_field(style, HOME_PAGE, field, Some(controls::CONTROL_WIDTH)),
        ));
    }
    sections.extend(visible(
        modal,
        category,
        "Browsing",
        Some("New browser tabs open to a blank page. Turn on the toggle to open them to a website instead."),
        true,
        browsing,
    ));

    let profiles: Vec<AnyElement> = muxy_core::store::browser_profiles()
        .into_iter()
        .map(|(id, name)| {
            controls::row(
                style,
                &name,
                inert_actions(style, &id, &["Rename", "Import", "Clear Data", "Delete"]),
            )
        })
        .collect();
    sections.extend(visible(
        modal,
        category,
        "Profiles",
        Some("Each profile keeps its own cookies, cache, and logins. Pick a profile per tab from the browser toolbar. Import brings an existing browser's cookies so tabs start signed in."),
        false,
        profiles,
    ));

    sections
}

pub(super) fn inert_actions(style: Style, id: &str, labels: &[&str]) -> AnyElement {
    let mut group = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(style.metrics.spacing3());
    for label in labels {
        group = group.child(controls::button(
            style,
            &format!("{id}-{label}"),
            label,
            false,
            |_, _, _| {},
        ));
    }
    group.into_any_element()
}
