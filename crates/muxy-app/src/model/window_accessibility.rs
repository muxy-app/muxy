//! What the operating system sees of the main window: a title naming the
//! active tab and project, and on macOS a text area while typing goes to a
//! terminal or the Composer. Time trackers, dictation, and automation tools
//! read these.

use gpui::{App, Context, Window};
#[cfg(target_os = "macos")]
use {
    gpui::{Focusable, SharedString},
    muxy_ui::tr,
};

use super::AppModel;

#[derive(Default)]
pub(crate) struct WindowAccessibility {
    title: String,
    #[cfg(target_os = "macos")]
    text_input: Option<SharedString>,
}

impl AppModel {
    pub(crate) fn sync_window_accessibility(&mut self, window: &mut Window, cx: &Context<Self>) {
        let title = self.window_title(cx);
        if title != self.window_accessibility.title {
            window.set_window_title(&title);
            self.window_accessibility.title = title;
        }
        #[cfg(target_os = "macos")]
        {
            let input = self.focused_text_input(window, cx);
            if input != self.window_accessibility.text_input {
                #[cfg(not(test))]
                muxy_ui::accessibility::set_text_input(
                    window,
                    input.as_ref().map(SharedString::as_str),
                );
                self.window_accessibility.text_input = input;
            }
        }
    }

    fn window_title(&self, cx: &App) -> String {
        let tab = self
            .active_tab()
            .and_then(|id| self.tab(id))
            .map_or("", |tab| self.webview_title(tab, cx));
        title(tab, &self.state.current_project().name)
    }

    #[cfg(target_os = "macos")]
    pub(super) fn focused_text_input(&self, window: &Window, cx: &App) -> Option<SharedString> {
        if self
            .composer
            .view
            .as_ref()
            .is_some_and(|composer| composer.focus_handle(cx).contains_focused(window, cx))
        {
            return Some(tr!("Composer"));
        }
        let pane = self.active_pane().and_then(|id| self.terminal(&id))?;
        pane.view
            .read(cx)
            .focus
            .contains_focused(window, cx)
            .then(|| tr!("Terminal"))
    }
}

fn title(tab: &str, project: &str) -> String {
    match (tab.trim(), project.trim()) {
        ("", "") => muxy_core::release::Channel::current().app_name().to_owned(),
        (tab, "") => tab.to_owned(),
        ("", project) => project.to_owned(),
        (tab, project) => format!("{tab} — {project}"),
    }
}
