use super::AppModel;
use muxy_ui::toast::ToastKind;

pub(crate) const TOAST_TRANSITION: std::time::Duration = std::time::Duration::from_millis(200);
const NOTICE_DURATION: std::time::Duration = std::time::Duration::from_secs(3);

pub(crate) struct Toast {
    pub(crate) kind: ToastKind,
    pub(crate) message: String,
    pub(crate) body: Option<String>,
    pub(crate) motion: muxy_ui::motion::VisibilityMotion,
    closing: bool,
    serial: u64,
}

impl Toast {
    fn matches(&self, kind: ToastKind, message: &str, body: Option<&str>, serial: u64) -> bool {
        !self.closing
            && self.kind == kind
            && self.message == message
            && self.body.as_deref() == body
            && self.serial == serial
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BannerKind {
    Error = 0,
    Configuration = 1,
}

impl AppModel {
    /// Problems stay until dismissed; notices hide on their own and never replace a problem.
    pub(crate) fn sync_toast(&mut self, cx: &mut gpui::Context<Self>) {
        self.sync_problem_toast(cx);
        self.sync_notice_toast(cx);
    }

    fn sync_problem_toast(&mut self, cx: &mut gpui::Context<Self>) {
        let Some((kind, message)) = self
            .visible_banner()
            .map(|(kind, message)| (kind, message.to_owned()))
        else {
            if self
                .problem_toast
                .as_ref()
                .is_some_and(|toast| !toast.closing)
            {
                self.problem_toast = None;
            }
            return;
        };
        let (kind, body) = match kind {
            BannerKind::Error => (ToastKind::Error, self.error_detail.clone()),
            BannerKind::Configuration => (ToastKind::Warning, None),
        };
        if self.problem_toast.as_ref().is_some_and(|toast| {
            toast.matches(kind, &message, body.as_deref(), self.problem_serial)
        }) {
            return;
        }
        self.problem_task = None;
        self.problem_toast = Some(Toast {
            kind,
            message,
            body,
            motion: muxy_ui::motion::VisibilityMotion::showing(
                TOAST_TRANSITION,
                cx.background_executor().clone(),
            ),
            closing: false,
            serial: self.problem_serial,
        });
    }

    fn sync_notice_toast(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(message) = self.notice.clone() else {
            if self
                .notice_toast
                .as_ref()
                .is_some_and(|toast| !toast.closing)
            {
                self.notice_toast = None;
                self.notice_task = None;
            }
            return;
        };
        let serial = self.notice_serial;
        if self.notice_toast.as_ref().is_some_and(|toast| {
            toast.matches(
                ToastKind::Success,
                &message,
                self.notice_body.as_deref(),
                serial,
            )
        }) {
            return;
        }
        self.notice_toast = Some(Toast {
            kind: ToastKind::Success,
            message,
            body: self.notice_body.clone(),
            motion: muxy_ui::motion::VisibilityMotion::showing(
                TOAST_TRANSITION,
                cx.background_executor().clone(),
            ),
            closing: false,
            serial,
        });
        self.notice_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(NOTICE_DURATION).await;
            let started_hiding = this.update(cx, |model, cx| {
                if model.notice_serial != serial {
                    return false;
                }
                model.notice = None;
                model.notice_body = None;
                if let Some(toast) = &mut model.notice_toast {
                    toast.closing = true;
                    toast.motion.hide();
                }
                cx.notify();
                true
            });
            if !matches!(started_hiding, Ok(true)) {
                return;
            }
            cx.background_executor().timer(TOAST_TRANSITION).await;
            let _ = this.update(cx, |model, cx| {
                if model
                    .notice_toast
                    .as_ref()
                    .is_some_and(|toast| toast.closing && toast.serial == serial)
                {
                    model.notice_toast = None;
                    model.notice_task = None;
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn visible_banner(&self) -> Option<(BannerKind, &str)> {
        [
            (BannerKind::Error, self.error.as_deref()),
            (
                BannerKind::Configuration,
                self.configuration_error.as_deref(),
            ),
        ]
        .into_iter()
        .find_map(|(kind, message)| {
            message
                .filter(|message| {
                    !message.trim().is_empty()
                        && self.dismissed_banners[kind as usize].as_deref() != Some(*message)
                })
                .map(|message| (kind, message))
        })
    }

    pub(crate) fn dismiss_banner(&mut self, kind: BannerKind) {
        self.dismissed_banners[kind as usize].clone_from(match kind {
            BannerKind::Error => &self.error,
            BannerKind::Configuration => &self.configuration_error,
        });
    }

    pub(crate) fn dismiss_problem(&mut self, cx: &mut gpui::Context<Self>) {
        let Some((kind, _)) = self.visible_banner() else {
            return;
        };
        self.dismiss_banner(kind);
        if let Some(toast) = &mut self.problem_toast {
            toast.closing = true;
            toast.motion.hide();
        }
        self.problem_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_TRANSITION).await;
            let _ = this.update(cx, |model, cx| {
                if model
                    .problem_toast
                    .as_ref()
                    .is_some_and(|toast| toast.closing)
                {
                    model.problem_toast = None;
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn set_banner_error(&mut self, error: Option<String>) {
        if self.error != error {
            self.dismissed_banners[BannerKind::Error as usize] = None;
        }
        self.error = error;
        self.error_detail = None;
    }

    /// Reports a failed user action; it shows even when an identical failure was dismissed.
    pub(crate) fn fail_detail(
        &mut self,
        title: String,
        detail: &str,
        cx: &mut gpui::Context<Self>,
    ) {
        self.set_banner_error(Some(title));
        self.error_detail = Some(detail.trim().to_owned()).filter(|detail| !detail.is_empty());
        self.dismissed_banners[BannerKind::Error as usize] = None;
        self.problem_serial = self.problem_serial.wrapping_add(1);
        cx.notify();
    }

    pub(crate) fn show_notice(&mut self, notice: String, cx: &mut gpui::Context<Self>) {
        self.show_toast(notice, None, cx);
    }

    pub(crate) fn show_toast(
        &mut self,
        title: String,
        body: Option<String>,
        cx: &mut gpui::Context<Self>,
    ) {
        self.notice_body = body
            .map(|body| body.trim().to_owned())
            .filter(|body| !body.is_empty());
        self.notice = Some(if title.trim().is_empty() {
            self.notice_body.take().unwrap_or_default()
        } else {
            title
        });
        self.notice_serial = self.notice_serial.wrapping_add(1);
        cx.notify();
    }

    pub(super) fn set_configuration_error(&mut self, error: Option<String>) {
        if self.configuration_error != error {
            self.dismissed_banners[BannerKind::Configuration as usize] = None;
        }
        self.configuration_error = error;
    }
}
