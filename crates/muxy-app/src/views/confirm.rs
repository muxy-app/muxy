use gpui::{AnyWindowHandle, AsyncApp, Context};
use muxy_ui::dialog::ConfirmationResponse;

use crate::model::AppModel;

pub(crate) const TITLE: &str = "Close Tab?";
pub(crate) const MESSAGE: &str =
    "A process is still running in this tab.\nAre you sure you want to close it?";

pub(crate) const PANE_TITLE: &str = "Close Pane?";
pub(crate) const PANE_MESSAGE: &str =
    "A process is still running in this pane.\nAre you sure you want to close it?";

impl AppModel {
    pub(crate) fn confirm_close(&mut self, tab: muxy_app_core::TabId, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let window = self.window;
        let (title, message) = if self.closing_multiple_tabs() {
            (
                "Close Tabs?",
                "A process is still running in these tabs.\nAre you sure you want to close them?",
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
                title,
                message,
                "Close",
                Some("Don't ask again"),
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

pub(crate) async fn prompt_server(
    window: AnyWindowHandle,
    restart: bool,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = if restart {
        "Restart Server?"
    } else {
        "Stop Server?"
    };
    let label = if restart { "Restart" } else { "Stop" };
    let message = "All running terminal sessions on this device will end. Saved terminal output and settings will remain.";
    server_prompt(window, title, label, message, cx).await
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
    let message = if scheduled {
        format!(
            "Muxy {version} will install and restart the app when all terminal sessions end. Currently {sessions} sessions are running, including idle shells and detached sessions."
        )
    } else if compatible {
        format!(
            "Install Muxy {version} and restart the app? Running terminals will continue. After the app reopens, restart the server from the update status, or let it update when all terminal sessions end."
        )
    } else {
        format!(
            "Muxy {version} requires a server restart. You can update automatically when all terminal sessions end, or end them now. Terminal panes will close. App-only panes and settings will remain."
        )
    };
    let labels: &[&str] = if scheduled {
        &[
            "Keep Waiting",
            "Update and End Sessions…",
            "Cancel Scheduled Update",
        ]
    } else if compatible {
        &["Update and Restart App", "Not Now"]
    } else {
        &[
            "Update When Sessions End",
            "Update and End Sessions…",
            "Not Now",
        ]
    };
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Info,
                "Muxy Update",
                Some(&message),
                labels,
                cx,
            )
        })
        .map_err(|error| error.to_string())?
        .await
        .map_err(|error| error.to_string())?;
    let choice = match (scheduled, compatible, answer) {
        (true, _, 2) => UpdateChoice::CancelSchedule,
        (true, _, 1) | (false, false, 1) => UpdateChoice::EndSessions,
        (false, true, 0) => UpdateChoice::Install,
        (false, false, 0) => UpdateChoice::Schedule,
        _ => UpdateChoice::Later,
    };
    if choice == UpdateChoice::EndSessions && !server_prompt(window, "Update and End All Sessions?", "Update and End Sessions", "All terminal processes on this device will end, including sessions used by other clients. Terminal panes will close. App-only panes and settings will remain.", cx).await? {
        return Ok(UpdateChoice::Later);
    }
    Ok(choice)
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
