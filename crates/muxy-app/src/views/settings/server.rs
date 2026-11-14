use super::slider::{Commit, Slider};
use super::{Category, SettingsEvent, SettingsView};
use crate::model::preferences::Change;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use muxy_ui::controls;
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::tr;

/// This computer's server at the top of the Servers page: how it is doing
/// with Connect, or Restart and Stop, then the settings of its sessions.
pub(super) fn rows(pane: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    let connected = pane.snapshot.connected;
    if let Some(description) = &pane.snapshot.server_update
        && pane.matches(Category::Servers, tr_key!("Server update version"))
    {
        rows.push(pane.note(description, false).into_any_element());
    }
    if pane.matches(Category::Servers, tr_key!("This computer's server")) {
        rows.push(pane.row(
            "server",
            tr_key!("This computer's server"),
            server_controls(pane, cx),
        ));
    }
    rows.extend(background_activity(pane, cx));
    if !connected {
        if pane.matches(Category::Servers, tr_key!("Server disconnected")) {
            let note = tr!(
                "Server disconnected. App preferences still work. Connect to edit server settings."
            );
            rows.push(pane.note(&note, false).into_any_element());
        }
        return rows;
    }
    if let Some(server) = &pane.snapshot.server {
        if pane.matches(
            Category::Servers,
            tr_key!("Default shell (executable path)"),
        ) {
            rows.push(pane.row(
                "default-shell",
                tr_key!("Default shell (executable path)"),
                pane.field("default-shell"),
            ));
        }
        if pane.matches(
            Category::Servers,
            tr_key!("History budget per session (MiB)"),
        ) {
            rows.push(history_budget(pane, server, cx));
        }
        if pane.matches(Category::Servers, tr_key!("Shell integration")) {
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
    } else if pane.matches(Category::Servers, tr_key!("Loading settings")) {
        let note = tr!("Loading this computer's server settings…");
        rows.push(pane.note(&note, false).into_any_element());
    }
    rows
}

/// Connect while the server is offline; Restart and Stop, which ask first,
/// while it runs.
fn server_controls(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let enabled = !pane.snapshot.server_busy;
    if !pane.snapshot.connected {
        return controls::button(
            pane.style(),
            "server-connect",
            controls::Button::secondary(&tr!("Connect")).enabled(enabled),
            cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Connect)),
        )
        .into_any_element();
    }
    let control = |label: &'static str, restart: bool, cx: &mut Context<SettingsView>| {
        controls::button(
            pane.style(),
            label,
            controls::Button::secondary(&translate(label)).enabled(enabled),
            cx.listener(move |_, _, _, cx| {
                cx.emit(SettingsEvent::ServerControl { restart });
            }),
        )
    };
    div()
        .flex()
        .flex_wrap()
        .gap(pane.style().metrics.spacing3())
        .child(control(tr_key!("Restart Server…"), true, cx))
        .child(control(tr_key!("Stop Server…"), false, cx))
        .into_any_element()
}

/// On macOS, terminals outlive the app only while Muxy may run in the
/// background, so this row opens the setting that allows it.
fn background_activity(pane: &SettingsView, cx: &mut Context<SettingsView>) -> Option<AnyElement> {
    let label = tr_key!("Keep terminals running after quitting");
    (cfg!(target_os = "macos") && pane.matches(Category::Servers, label)).then(|| {
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
