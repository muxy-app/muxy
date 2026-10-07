use super::{Category, Change, SettingsEvent, SettingsView};
use gpui::{AnyElement, Context, IntoElement};
use muxy_ui::controls;
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::tr;

pub(super) fn rows(pane: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    let connected = pane.snapshot.connected;
    if let Some(description) = &pane.snapshot.server_update
        && pane.matches(Category::Server, tr_key!("Server update version"))
    {
        rows.push(pane.note(description, false).into_any_element());
    }
    if pane.matches(Category::Server, tr_key!("Current device")) {
        rows.push(
            pane.row(
                "server",
                tr_key!("Current device"),
                controls::button(
                    pane.style(),
                    "server-read",
                    &if connected {
                        tr!("Reload settings")
                    } else {
                        tr!("Connect")
                    },
                    !pane.snapshot.server_busy,
                    cx.listener(move |_, _, _, cx| {
                        cx.emit(if connected {
                            SettingsEvent::ReadServer
                        } else {
                            SettingsEvent::Connect
                        });
                    }),
                )
                .into_any_element(),
            ),
        );
    }
    if !connected {
        if pane.matches(Category::Server, tr_key!("Server disconnected")) {
            let note = tr!(
                "Server disconnected. App preferences still work. Connect to edit server settings."
            );
            rows.push(pane.note(&note, false).into_any_element());
        }
        return rows;
    }
    if let Some(server) = &pane.snapshot.server {
        for (id, label) in [
            ("default-shell", tr_key!("Default shell (executable path)")),
            (
                "history-budget",
                tr_key!("History budget per session (MiB)"),
            ),
        ] {
            if pane.matches(Category::Server, label) {
                rows.push(pane.row(id, label, pane.field(id)));
            }
        }
        if pane.matches(Category::Server, tr_key!("Shell integration")) {
            rows.push(pane.row(
                "shell-integration",
                tr_key!("Shell integration"),
                pane.toggle(
                    "shell-integration",
                    server.shell_integration,
                    Change::ShellIntegration(!server.shell_integration),
                    cx,
                ),
            ));
        }
    } else if pane.matches(Category::Server, tr_key!("Loading settings")) {
        let note = tr!("Load the current device's settings to edit them.");
        rows.push(pane.note(&note, false).into_any_element());
    }
    rows.extend(sandbox_rows(pane));
    if pane.matches(Category::Server, tr_key!("Sandboxed terminals")) {
        rows.push(pane.note(&tr!("Sandboxed terminals are experimental. Settings apply to new terminals. Unrestricted networking is unavailable because it exposes host control sockets."), false).into_any_element());
    }
    for (restart, label) in [
        (false, tr_key!("Stop Server")),
        (true, tr_key!("Restart Server")),
    ] {
        if pane.matches(Category::Server, label) {
            rows.push(
                pane.row(
                    label,
                    label,
                    controls::button(
                        pane.style(),
                        label,
                        &translate(label),
                        !pane.snapshot.server_busy,
                        cx.listener(move |_, _, _, cx| {
                            cx.emit(SettingsEvent::ServerControl { restart });
                        }),
                    )
                    .into_any_element(),
                ),
            );
        }
    }
    rows
}

fn sandbox_rows(pane: &SettingsView) -> Vec<AnyElement> {
    if pane.snapshot.server.is_none() {
        return Vec::new();
    }
    [
        ("sandbox-executable", tr_key!("Sandbox: nono executable")),
        (
            "sandbox-network",
            tr_key!("Sandbox network: blocked or domains"),
        ),
        (
            "sandbox-domains",
            tr_key!("Sandbox approved domains (comma separated)"),
        ),
        (
            "sandbox-tools",
            tr_key!("Sandbox read-only tool paths (semicolon separated)"),
        ),
        (
            "sandbox-environment",
            tr_key!("Sandbox approved environment names (comma separated)"),
        ),
    ]
    .into_iter()
    .filter(|(_, label)| pane.matches(Category::Server, label))
    .map(|(id, label)| pane.row(id, label, pane.field(id)))
    .collect()
}
