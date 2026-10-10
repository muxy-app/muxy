use gpui::{AnyWindowHandle, AsyncApp, Context};
use muxy_ui::dialog::ConfirmationResponse;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

use crate::model::AppModel;

pub(crate) const TITLE: &str = tr_key!("Close Tab?");
pub(crate) const MESSAGE: &str =
    tr_key!("A process is still running in this tab.\nAre you sure you want to close it?");

pub(crate) const PANE_TITLE: &str = tr_key!("Close Pane?");
pub(crate) const PANE_MESSAGE: &str =
    tr_key!("A process is still running in this pane.\nAre you sure you want to close it?");

impl AppModel {
    pub(crate) fn confirm_close(&mut self, tab: muxy_app_core::TabId, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let window = self.window;
        let (title, message) = if self.closing_multiple_tabs() {
            (
                tr_key!("Close Tabs?"),
                tr_key!(
                    "A process is still running in these tabs.\nAre you sure you want to close them?"
                ),
            )
        } else if self.closing_one_pane() {
            (PANE_TITLE, PANE_MESSAGE)
        } else {
            (TITLE, MESSAGE)
        };
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = prompt(window, title, message, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_close_prompt(tab, response, cx);
            });
        }));
    }
}

#[cfg(not(test))]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, _| {
            muxy_ui::dialog::confirm(
                window,
                &muxy_ui::l10n::translate(title),
                &muxy_ui::l10n::translate(message),
                &tr!("Close"),
                Some(&tr!("Don't ask again")),
                move |response| {
                    let _ = sender.try_send(response);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or_default())
}

#[cfg(test)]
pub(crate) const CLOSE_WITHOUT_ASKING: &str = "Close and don't ask again";

#[cfg(test)]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                title,
                Some(message),
                &["Close", "Cancel", CLOSE_WITHOUT_ASKING],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(match answer.await {
        Ok(0) => ConfirmationResponse::Confirmed {
            dont_ask_again: false,
        },
        Ok(2) => ConfirmationResponse::Confirmed {
            dont_ask_again: true,
        },
        _ => ConfirmationResponse::Cancelled,
    })
}

/// Stops or restarts a server; `name` is another computer's, `None` this one's.
pub(crate) async fn prompt_server(
    window: AnyWindowHandle,
    restart: bool,
    name: Option<&str>,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = match (restart, name) {
        (true, None) => tr!("Restart Server?"),
        (false, None) => tr!("Stop Server?"),
        (true, Some(name)) => tr!("Restart the Server on %@?", name),
        (false, Some(name)) => tr!("Stop the Server on %@?", name),
    };
    let label = if restart { tr!("Restart") } else { tr!("Stop") };
    let message = match name {
        Some(name) => tr!(
            "All running terminal sessions on %@ will end. Saved terminal output and settings will remain.",
            name
        ),
        None => tr!(
            "All running terminal sessions on this device will end. Saved terminal output and settings will remain."
        ),
    };
    server_prompt(window, &title, &label, &message, cx).await
}

pub(crate) async fn prompt_install_server(
    window: AnyWindowHandle,
    name: &str,
    source: &crate::remote_install::Source,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = tr!("Install muxy-server on %@?", name);
    let message = source.description(name);
    server_prompt(window, &title, &tr!("Install"), &message, cx).await
}

pub(crate) async fn prompt_forget_server(
    window: AnyWindowHandle,
    name: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = tr!("Forget %@?", name);
    let message = tr!("Its projects and layouts leave this app; nothing on the server changes.");
    server_prompt(window, &title, &tr!("Forget"), &message, cx).await
}

pub(crate) async fn prompt_revoke(
    window: AnyWindowHandle,
    device: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = tr!("Revoke %@?", device);
    let message = tr!("The device disconnects now and must be paired again to reconnect.");
    server_prompt(window, &title, &tr!("Revoke"), &message, cx).await
}

pub(crate) async fn prompt_reset_terminal(
    window: AnyWindowHandle,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = tr!("Reset Terminal Settings?");
    let message =
        tr!("Every setting on the Terminal page, including key bindings, returns to its default.");
    server_prompt(window, &title, &tr!("Reset"), &message, cx).await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UpdateChoice {
    Install,
    Schedule,
    CancelSchedule,
    EndSessions,
    Later,
}

pub(crate) async fn prompt_update(
    window: AnyWindowHandle,
    version: &str,
    compatible: bool,
    scheduled: bool,
    sessions: usize,
    cx: &mut AsyncApp,
) -> Result<UpdateChoice, String> {
    if compatible && !scheduled {
        return prompt_install(window, version, cx).await;
    }
    let message = if scheduled {
        tr!(
            "Muxy %@ will install and restart the app when all terminal sessions end. Currently %lld sessions are running, including idle shells and detached sessions.",
            version,
            sessions
        )
    } else {
        tr!(
            "Muxy %@ requires a server restart. You can update automatically when all terminal sessions end, or end them now. Terminal panes will close. App-only panes and settings will remain.",
            version
        )
    };
    let labels = if scheduled {
        [
            tr!("Keep Waiting"),
            tr!("Update and End Sessions…"),
            tr!("Cancel Scheduled Update"),
        ]
    } else {
        [
            tr!("Update When Sessions End"),
            tr!("Update and End Sessions…"),
            tr!("Not Now"),
        ]
    };
    let labels = labels.each_ref().map(gpui::SharedString::as_str);
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Info,
                &tr!("Muxy Update"),
                Some(&message),
                &labels,
                cx,
            )
        })
        .map_err(|error| error.to_string())?
        .await
        .map_err(|error| error.to_string())?;
    let choice = match (scheduled, answer) {
        (true, 2) => UpdateChoice::CancelSchedule,
        (_, 1) => UpdateChoice::EndSessions,
        (false, 0) => UpdateChoice::Schedule,
        _ => UpdateChoice::Later,
    };
    if choice == UpdateChoice::EndSessions && !server_prompt(window, &tr!("Update and End All Sessions?"), &tr!("Update and End Sessions"), &tr!("All terminal processes on this device will end, including sessions used by other clients. Terminal panes will close. App-only panes and settings will remain."), cx).await? {
        return Ok(UpdateChoice::Later);
    }
    Ok(choice)
}

const INSTALL: &str = tr_key!("Update and Restart App");

async fn prompt_install(
    window: AnyWindowHandle,
    version: &str,
    cx: &mut AsyncApp,
) -> Result<UpdateChoice, String> {
    let message = tr!(
        "Install Muxy %@ and restart the app? Running terminals will continue, and the server will update when all terminal sessions end. Restarting the server now ends all terminal sessions on this device, including sessions used by other clients.",
        version
    );
    Ok(match install_prompt(window, &message, cx).await? {
        Some(true) => UpdateChoice::EndSessions,
        Some(false) => UpdateChoice::Install,
        None => UpdateChoice::Later,
    })
}

/// Reports whether the server should restart too, or `None` when declined.
#[cfg(not(test))]
async fn install_prompt(
    window: AnyWindowHandle,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<Option<bool>, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, _| {
            muxy_ui::dialog::confirm_with_checkbox(
                window,
                &tr!("Muxy Update"),
                message,
                [&muxy_ui::l10n::translate(INSTALL), &tr!("Not Now")],
                &tr!("Also restart the server"),
                move |restart_server| {
                    let _ = sender.try_send(restart_server);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or_default())
}

#[cfg(test)]
pub(crate) const INSTALL_AND_RESTART_SERVER: &str = "Update and Restart App and Server";

#[cfg(test)]
async fn install_prompt(
    window: AnyWindowHandle,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<Option<bool>, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Info,
                "Muxy Update",
                Some(message),
                &[INSTALL, "Not Now", INSTALL_AND_RESTART_SERVER],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(match answer.await {
        Ok(0) => Some(false),
        Ok(2) => Some(true),
        _ => None,
    })
}

#[cfg(not(test))]
async fn server_prompt(
    window: AnyWindowHandle,
    title: &str,
    label: &str,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, _| {
            muxy_ui::dialog::confirm(window, title, message, label, None, move |response| {
                let _ = sender.try_send(matches!(response, ConfirmationResponse::Confirmed { .. }));
            })
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or(false))
}

#[cfg(test)]
async fn server_prompt(
    window: AnyWindowHandle,
    title: &str,
    label: &str,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                title,
                Some(message),
                &[label, "Cancel"],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(answer.await == Ok(0))
}
