use gpui::{AsyncApp, Context};
use muxy_ui::dialog::{ConfirmationResponse, Parent, Question, ask, choose};
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

use crate::model::AppModel;
use crate::model::dialogs::DialogId;

pub(crate) const TITLE: &str = tr_key!("Close Tab?");
pub(crate) const MESSAGE: &str =
    tr_key!("A process is still running in this tab.\nAre you sure you want to close it?");

pub(crate) const PANE_TITLE: &str = tr_key!("Close Pane?");
pub(crate) const PANE_MESSAGE: &str =
    tr_key!("A process is still running in this pane.\nAre you sure you want to close it?");

impl AppModel {
    pub(crate) fn confirm_close(
        &mut self,
        tab: muxy_app_core::TabId,
        cx: &mut Context<Self>,
    ) -> DialogId {
        self.dismiss_overlay(cx);
        let (title, message, action) = if self.restarting_terminal() {
            (
                tr_key!("Restart Terminal?"),
                tr_key!(
                    "A process is still running in this pane.\nRestarting the terminal ends it."
                ),
                tr_key!("Restart"),
            )
        } else if self.closing_multiple_tabs() {
            (
                tr_key!("Close Tabs?"),
                tr_key!(
                    "A process is still running in these tabs.\nAre you sure you want to close them?"
                ),
                tr_key!("Close"),
            )
        } else if self.closing_one_pane() {
            (PANE_TITLE, PANE_MESSAGE, tr_key!("Close"))
        } else {
            (TITLE, MESSAGE, tr_key!("Close"))
        };
        self.ask_dialog(
            self.window,
            async move |parent, cx| prompt(parent, title, message, action, cx).await,
            move |model, response, cx| model.finish_close_confirmation(tab, response, cx),
            cx,
        )
    }
}

async fn prompt(
    parent: &Parent,
    title: &'static str,
    message: &'static str,
    action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let question = Question::Confirm {
        title: &muxy_ui::l10n::translate(title),
        message: &muxy_ui::l10n::translate(message),
        confirm: &muxy_ui::l10n::translate(action),
        checkbox: Some(&tr!("Don't ask again")),
    };
    let answer = ask(parent, question, cx)
        .await
        .map_err(|error| error.to_string())?;
    Ok(answer.map_or(ConfirmationResponse::Cancelled, |answer| {
        ConfirmationResponse::Confirmed {
            dont_ask_again: answer.ticked(),
        }
    }))
}

/// Stops or restarts a server; `name` is another computer's, `None` this one's.
pub(crate) async fn prompt_server(
    parent: &Parent,
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
    server_prompt(parent, &title, &label, &message, cx).await
}

pub(crate) async fn prompt_install_server(
    parent: &Parent,
    name: &str,
    source: &crate::remote_install::Source,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = tr!("Install muxy-server on %@?", name);
    let message = source.description(name);
    server_prompt(parent, &title, &tr!("Install"), &message, cx).await
}

pub(crate) async fn prompt_forget_server(
    parent: &Parent,
    name: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = tr!("Forget %@?", name);
    let message = tr!("Its projects and layouts leave this app; nothing on the server changes.");
    server_prompt(parent, &title, &tr!("Forget"), &message, cx).await
}

pub(crate) async fn prompt_forget_servers(
    parent: &Parent,
    count: usize,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = tr!("Forget %lld Servers?", count);
    let message = tr!("Their projects and layouts leave this app; nothing on the servers changes.");
    server_prompt(parent, &title, &tr!("Forget"), &message, cx).await
}

pub(crate) async fn prompt_revoke(
    parent: &Parent,
    device: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = tr!("Revoke %@?", device);
    let message = tr!("The device disconnects now and must be paired again to reconnect.");
    server_prompt(parent, &title, &tr!("Revoke"), &message, cx).await
}

pub(crate) async fn prompt_reset_terminal(
    parent: &Parent,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = tr!("Reset Terminal Settings?");
    let message =
        tr!("Every setting on the Terminal page, including key bindings, returns to its default.");
    server_prompt(parent, &title, &tr!("Reset"), &message, cx).await
}

pub(crate) const BACKGROUND_TITLE: &str = tr_key!("Allow Muxy to Run in the Background?");
pub(crate) const OPEN_LOGIN_ITEMS: &str = tr_key!("Open Login Items");

/// Whether to open the setting that keeps terminals running after the app
/// quits.
pub(crate) async fn prompt_background_activity(
    parent: &Parent,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let message = tr!(
        "macOS ended your terminals when Muxy quit, because Muxy isn't allowed to run in the background. To keep them running, allow Muxy to run in the background in Login Items settings."
    );
    let labels = [muxy_ui::l10n::translate(OPEN_LOGIN_ITEMS), tr!("Not Now")];
    let labels = labels.each_ref().map(gpui::SharedString::as_str);
    let title = muxy_ui::l10n::translate(BACKGROUND_TITLE);
    let answer = choose(parent, &title, &message, &labels, cx)
        .await
        .map_err(|error| error.to_string())?;
    Ok(answer == Some(0))
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
    parent: &Parent,
    version: &str,
    compatible: bool,
    scheduled: bool,
    sessions: usize,
    cx: &mut AsyncApp,
) -> Result<UpdateChoice, String> {
    if compatible && !scheduled {
        return prompt_install(parent, version, cx).await;
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
    let answer = choose(parent, &tr!("Muxy Update"), &message, &labels, cx)
        .await
        .map_err(|error| error.to_string())?;
    let choice = match (scheduled, answer) {
        (true, Some(2)) => UpdateChoice::CancelSchedule,
        (_, Some(1)) => UpdateChoice::EndSessions,
        (false, Some(0)) => UpdateChoice::Schedule,
        _ => UpdateChoice::Later,
    };
    if choice == UpdateChoice::EndSessions && !server_prompt(parent, &tr!("Update and End All Sessions?"), &tr!("Update and End Sessions"), &tr!("All terminal processes on this device will end, including sessions used by other clients. Terminal panes will close. App-only panes and settings will remain."), cx).await? {
        return Ok(UpdateChoice::Later);
    }
    Ok(choice)
}

const INSTALL: &str = tr_key!("Update and Restart App");

async fn prompt_install(
    parent: &Parent,
    version: &str,
    cx: &mut AsyncApp,
) -> Result<UpdateChoice, String> {
    let message = tr!(
        "Install Muxy %@ and restart the app? Running terminals will continue, and the server will update when all terminal sessions end. Restarting the server now ends all terminal sessions on this device, including sessions used by other clients.",
        version
    );
    Ok(match install_prompt(parent, &message, cx).await? {
        Some(true) => UpdateChoice::EndSessions,
        Some(false) => UpdateChoice::Install,
        None => UpdateChoice::Later,
    })
}

/// Reports whether the server should restart too, or `None` when declined.
async fn install_prompt(
    parent: &Parent,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<Option<bool>, String> {
    let question = Question::Choose {
        title: &tr!("Muxy Update"),
        message,
        confirm: &muxy_ui::l10n::translate(INSTALL),
        decline: &tr!("Not Now"),
        checkbox: &tr!("Also restart the server"),
    };
    let answer = ask(parent, question, cx)
        .await
        .map_err(|error| error.to_string())?;
    Ok(answer.map(|answer| answer.ticked()))
}

async fn server_prompt(
    parent: &Parent,
    title: &str,
    label: &str,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let question = Question::Confirm {
        title,
        message,
        confirm: label,
        checkbox: None,
    };
    let answer = ask(parent, question, cx)
        .await
        .map_err(|error| error.to_string())?;
    Ok(answer.is_some())
}
