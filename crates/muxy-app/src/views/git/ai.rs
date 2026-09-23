use gpui::{AnyWindowHandle, AsyncApp};
use muxy_protocol::ProjectId;
use muxy_ui::controls::Style;

use crate::ai::Provider;
use crate::repository_actions::Action;

/// Captured before confirmation and revalidated before any repository changes.
pub(crate) struct AiConfirmation {
    pub(crate) project: ProjectId,
    pub(crate) action: Action,
    pub(crate) provider: Provider,
    pub(crate) branch: String,
    pub(crate) head: Option<String>,
}

impl AiConfirmation {
    fn title(&self) -> String {
        match self.action {
            Action::Commit => format!("Commit and push to \"{}\"?", self.branch),
            Action::CreatePullRequest => {
                format!("Create a pull request from \"{}\"?", self.branch)
            }
        }
    }

    fn message(&self) -> String {
        match self.action {
            Action::Commit => format!(
                "Muxy will stage all changes, ask {} for a commit message, then commit and push to \"{}\".",
                self.provider.name, self.branch
            ),
            Action::CreatePullRequest => format!(
                "Muxy will stage all changes, ask {} for a branch name, title, and summary, then create the branch, commit, push, and open the pull request on GitHub.",
                self.provider.name
            ),
        }
    }

    #[cfg(not(test))]
    pub(crate) async fn prompt(
        &self,
        window: AnyWindowHandle,
        style: Style<'_>,
        cx: &mut AsyncApp,
    ) -> Result<Option<String>, String> {
        let (sender, receiver) = async_channel::bounded(1);
        let _dialog = window
            .update(cx, |_, window, _| {
                muxy_ui::dialog::confirm_with_prompt(
                    window,
                    &self.title(),
                    &self.message(),
                    self.action.settings_title(),
                    style,
                    move |response| {
                        let _ = sender.try_send(response);
                    },
                )
            })
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        Ok(receiver.recv().await.ok().flatten())
    }

    #[cfg(test)]
    pub(crate) async fn prompt(
        &self,
        window: AnyWindowHandle,
        _style: Style<'_>,
        cx: &mut AsyncApp,
    ) -> Result<Option<String>, String> {
        let response = window
            .update(cx, |_, window, cx| {
                window.prompt(
                    gpui::PromptLevel::Info,
                    &self.title(),
                    Some(&self.message()),
                    &[self.action.settings_title(), "Cancel"],
                    cx,
                )
            })
            .map_err(|error| error.to_string())?;
        Ok((response.await == Ok(0)).then(String::new))
    }
}
