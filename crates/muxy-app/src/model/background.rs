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
    pub(super) fn explain_background_stop(&self, cx: &mut Context<Self>) {
        let window = self.window;
        cx.spawn(async move |model, cx| {
            if crate::views::confirm::prompt_background_activity(window, cx).await == Ok(true) {
                let _ = model.update(cx, Self::open_login_items);
            }
        })
        .detach();
    }
}
