use super::slider::{Commit, Slider};
use super::{Category, SettingsEvent, SettingsView};
use crate::model::preferences::Change;
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
                    controls::Button::secondary(&if connected {
                        tr!("Reload settings")
                    } else {
                        tr!("Connect")
                    })
                    .enabled(!pane.snapshot.server_busy),
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
    rows.extend(background_activity(pane, cx));
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
        if pane.matches(Category::Server, tr_key!("Default shell (executable path)")) {
            rows.push(pane.row(
                "default-shell",
                tr_key!("Default shell (executable path)"),
                pane.field("default-shell"),
            ));
        }
        if pane.matches(
            Category::Server,
            tr_key!("History budget per session (MiB)"),
        ) {
            rows.push(history_budget(pane, server, cx));
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
                        controls::Button::secondary(&translate(label))
                            .enabled(!pane.snapshot.server_busy),
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

/// On macOS, terminals outlive the app only while Muxy may run in the
/// background, so this row opens the setting that allows it.
fn background_activity(pane: &SettingsView, cx: &mut Context<SettingsView>) -> Option<AnyElement> {
    let label = tr_key!("Keep terminals running after quitting");
    (cfg!(target_os = "macos") && pane.matches(Category::Server, label)).then(|| {
        pane.row(
            "background-activity",
            label,
            controls::button(
                pane.style(),
                "background-activity",
                controls::Button::secondary(&translate(crate::views::confirm::OPEN_LOGIN_ITEMS)),
                cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::OpenLoginItems)),
            )
            .into_any_element(),
        )
    })
}

fn history_budget(
    pane: &SettingsView,
    server: &muxy_protocol::ServerSettingsDoc,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    #[allow(
        clippy::cast_precision_loss,
        reason = "The budget is at most 65536 MiB"
    )]
    let budget = (server.history_budget_bytes / (1024 * 1024)) as f32;
    pane.row(
        "history-budget",
        tr_key!("History budget per session (MiB)"),
        pane.slider_field(
            Slider::new("history-budget", (0.0, 65536.0), 1.0, "", Commit::Field),
            budget,
            cx,
        ),
    )
}
