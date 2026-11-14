use std::time::Duration;

use gpui::Context;
use muxy_core::release::Version;

use super::AppModel;

impl AppModel {
    /// Sends the anonymous daily ping from installed releases, checking every
    /// hour so a Muxy left open for days still counts each day. The setting is
    /// read before each check, so turning it off stops the next one.
    pub(super) fn start_usage_ping(&self, cx: &mut Context<Self>) {
        if cfg!(test)
            || Version::current().is_none()
            || crate::usage_ping::declined(std::env::var("DO_NOT_TRACK").ok().as_deref())
        {
            return;
        }
        let record = self.path.with_file_name("usage-ping");
        cx.spawn(async move |model, cx| {
            loop {
                let Ok(enabled) = model.update(cx, |model, _| model.settings.privacy.usage_ping)
                else {
                    break;
                };
                if enabled {
                    let record = record.clone();
                    let _ = cx
                        .background_executor()
                        .spawn(async move { crate::usage_ping::send_if_due(&record) })
                        .await;
                }
                cx.background_executor()
                    .timer(Duration::from_secs(3600))
                    .await;
            }
        })
        .detach();
    }
}
