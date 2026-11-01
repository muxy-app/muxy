//! On macOS, terminals outlive the app only while Muxy may run in the
//! background. Otherwise macOS ends them once the app quits.

use gpui::Context;

use super::AppModel;

/// The System Settings pane whose Background App Activity list allows Muxy.
const LOGIN_ITEMS: &str = "x-apple.systempreferences:com.apple.LoginItems-Settings.extension";

impl AppModel {
    pub(crate) fn open_login_items(&mut self, cx: &mut Context<Self>) {
        self.open_externally(LOGIN_ITEMS.into(), None, cx);
    }

    /// Explains why the terminals ended after the app last quit, and offers to
    /// open the setting that keeps them running.
    pub(super) fn explain_background_stop(&mut self, cx: &mut Context<Self>) {
        self.ask_dialog(
            self.window,
            async |parent, cx| crate::views::confirm::prompt_background_activity(parent, cx).await,
            |model, open, cx| {
                if open == Ok(true) {
                    model.open_login_items(cx);
                }
            },
            cx,
        );
    }
}
