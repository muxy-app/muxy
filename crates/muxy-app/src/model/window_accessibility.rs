//! What the operating system sees of the main window's title: the active tab
//! and project. Time trackers and automation tools read it.

use gpui::{App, Context, Window};

use super::AppModel;

#[derive(Default)]
pub(crate) struct WindowAccessibility {
    title: String,
}

impl AppModel {
    pub(crate) fn sync_window_accessibility(&mut self, window: &mut Window, cx: &Context<Self>) {
        let title = self.window_title(cx);
        if title != self.window_accessibility.title {
            window.set_window_title(&title);
            self.window_accessibility.title = title;
        }
    }

    fn window_title(&self, cx: &App) -> String {
        let tab = self
            .active_tab()
            .and_then(|id| self.tab(id))
            .map_or("", |tab| self.webview_title(tab, cx));
        title(tab, &self.state.current_project().name)
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
