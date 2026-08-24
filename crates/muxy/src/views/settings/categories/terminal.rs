use super::*;

pub(super) fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let category = Category::Terminal;
    let style = modal.style();
    let mut sections = Vec::new();

    sections.extend(visible(
        modal,
        category,
        "Selection",
        Some(
            "When enabled, releasing the mouse after selecting text in the terminal copies it to the clipboard.",
        ),
        true,
        vec![toggle_row(
            style,
            "Auto-copy selected text",
            "muxy.general.autoCopyTerminalSelection",
            false,
            cx,
        )],
    ));

    sections.extend(visible(
        modal,
        category,
        "Tabs",
        None,
        true,
        vec![toggle_row(
            style,
            "Confirm before closing a tab with a running process",
            "muxy.tabs.confirmCloseRunningProcess",
            true,
            cx,
        )],
    ));

    sections.extend(visible(
        modal,
        category,
        "Background sessions",
        Some(
            "Runs each new terminal in a separate background process, the way tmux does. Quitting Muxy leaves those terminals running and reopening reconnects them with their recent output. Closing a tab still ends its session. Use Send to Background from a terminal's context menu to close an eligible tab without stopping its processes. Terminals that are already open keep their current behaviour.",
        ),
        true,
        vec![toggle_row(
            style,
            "Run new terminals in the background",
            "muxy.terminalPersistentSession.enabled",
            false,
            cx,
        )],
    ));

    sections.extend(visible(
        modal,
        category,
        "Memory",
        Some(
            "Frees an idle terminal you are not actively using to reclaim memory, including visible split panes that are not focused. It reopens in the same folder when you return. Tabs running a process or a full-screen app are never touched.",
        ),
        false,
        vec![
            toggle_row(
                style,
                "Free idle inactive terminals",
                OFFLINE_ENABLED,
                false,
                cx,
            ),
            idle_timeout_row(modal, cx),
        ],
    ));

    sections
}

const IDLE_TIMEOUTS: [(f64, &str); 8] = [
    (10.0, "10 seconds"),
    (30.0, "30 seconds"),
    (60.0, "1 minute"),
    (120.0, "2 minutes"),
    (300.0, "5 minutes"),
    (600.0, "10 minutes"),
    (900.0, "15 minutes"),
    (1800.0, "30 minutes"),
];

fn idle_timeout_row(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> AnyElement {
    let style = modal.style();
    let enabled = settings::bool_value(OFFLINE_ENABLED, false);
    let stored = settings::f64_value(IDLE_THRESHOLD, 300.0);
    let closest = IDLE_TIMEOUTS
        .iter()
        .min_by(|left, right| (left.0 - stored).abs().total_cmp(&(right.0 - stored).abs()))
        .map(|entry| entry.1)
        .unwrap_or("5 minutes");
    let choices = IDLE_TIMEOUTS
        .iter()
        .map(|(_, label)| Choice {
            value: (*label).to_owned(),
            label: (*label).to_owned(),
            enabled,
        })
        .collect();

    let control = controls::picker(
        style,
        IDLE_THRESHOLD,
        choices,
        closest,
        enabled && modal.open_picker() == Some(IDLE_THRESHOLD),
        cx.listener(move |modal: &mut SettingsModal, _, _, cx| {
            if settings::bool_value(OFFLINE_ENABLED, false) {
                modal.toggle_picker(IDLE_THRESHOLD, cx);
            }
        }),
        cx.listener(|modal: &mut SettingsModal, value: &SharedString, _, cx| {
            let Some(seconds) = IDLE_TIMEOUTS
                .iter()
                .find(|(_, label)| *label == value.as_ref())
                .map(|(seconds, _)| *seconds)
            else {
                return;
            };
            let number = serde_json::Number::from_f64(seconds);
            modal.write(
                IDLE_THRESHOLD,
                number.map_or(Value::Null, Value::Number),
                cx,
            );
        }),
    );

    controls::row(
        style,
        "Free after idle for",
        div()
            .when(!enabled, |element: gpui::Div| element.opacity(0.4))
            .child(control)
            .into_any_element(),
    )
}
