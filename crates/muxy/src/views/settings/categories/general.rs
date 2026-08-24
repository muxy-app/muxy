use super::*;

pub(super) fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let category = Category::General;
    let style = modal.style();
    let mut sections = Vec::new();

    sections.extend(visible(
        modal,
        category,
        "Updates",
        Some(
            "The Beta channel ships every change merged to main and may be unstable. Switch back to Stable to receive only tagged releases.",
        ),
        true,
        vec![
            picker_row(
                modal,
                "Update channel",
                "muxy.update.channel",
                "stable",
                vec![Choice::new("stable", "Stable"), Choice::new("beta", "Beta")],
                cx,
            ),
            toggle_row(
                style,
                "Install Downloaded Updates on Quit",
                "SUAutomaticallyUpdate",
                true,
                cx,
            ),
        ],
    ));

    sections.extend(visible(
        modal,
        category,
        "Quit",
        None,
        true,
        vec![toggle_row(
            style,
            "Confirm before quitting Muxy",
            "muxy.app.confirmQuit",
            true,
            cx,
        )],
    ));

    sections.extend(visible(
        modal,
        category,
        "Diagnostics",
        Some(
            "Crash reports are sent only with your permission. Performance samples record CPU, memory, profiler uptime, app and macOS versions, device architecture, and timestamps once per minute. They stay on this Mac unless you share the file. Project paths, file contents, terminal output, and commands are never recorded.",
        ),
        false,
        vec![
            consent_row(style, cx),
            toggle_row(
                style,
                "Record anonymous performance samples",
                "diagnostics.profiler.enabled",
                false,
                cx,
            ),
            controls::row(
                style,
                "Profiler data",
                controls::button(
                    style,
                    "profiler-reveal",
                    "Reveal in Finder",
                    false,
                    |_, _, _| {},
                ),
            ),
        ],
    ));

    sections
}

fn consent_row(style: Style, cx: &mut Context<SettingsModal>) -> AnyElement {
    const KEY: &str = "muxy.sentry.consent";
    let allowed = settings::string_value(KEY, "") == "allowed";
    controls::row(
        style,
        "Send anonymous crash reports",
        controls::toggle(
            style,
            KEY,
            allowed,
            cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
                let allowed = settings::string_value(KEY, "") == "allowed";
                let next = if allowed { "denied" } else { "allowed" };
                modal.write(KEY, Value::String(next.to_owned()), cx);
            }),
        ),
    )
}
