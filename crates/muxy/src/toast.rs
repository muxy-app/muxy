use gpui::{Context, Task};
use std::time::Duration;

pub const TOAST_DURATION: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToastTone {
    Success,
    Warning,
    Error,
    Info,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToastAction {
    NavigateNotification(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToastContent {
    pub title: String,
    pub body: Option<String>,
    pub tone: ToastTone,
    pub action: Option<ToastAction>,
}

impl ToastContent {
    pub fn new(
        title: impl Into<String>,
        body: impl Into<String>,
        tone: ToastTone,
        action: Option<ToastAction>,
    ) -> Self {
        let body = body.into();
        let body = (!body.trim().is_empty()).then_some(body);
        Self {
            title: title.into(),
            body,
            tone,
            action,
        }
    }

    pub fn accessibility_label(&self) -> String {
        self.body.as_ref().map_or_else(
            || self.title.clone(),
            |body| format!("{}, {body}", self.title),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToastPosition {
    TopCenter,
    TopRight,
    BottomCenter,
    BottomRight,
}

impl ToastPosition {
    pub fn from_setting(value: &str) -> Self {
        match value {
            "Top Right" => Self::TopRight,
            "Bottom Center" => Self::BottomCenter,
            "Bottom Right" => Self::BottomRight,
            "Top Center" => Self::TopCenter,
            _ => Self::TopCenter,
        }
    }

    pub fn is_top(self) -> bool {
        matches!(self, Self::TopCenter | Self::TopRight)
    }

    pub fn is_centered(self) -> bool {
        matches!(self, Self::TopCenter | Self::BottomCenter)
    }

    pub fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToastOrigin {
    Notification,
    Feedback,
}

impl ToastOrigin {
    pub fn should_present(self, notification_toasts_enabled: bool) -> bool {
        matches!(self, Self::Feedback) || notification_toasts_enabled
    }
}

#[derive(Default)]
pub struct ToastState {
    current: Option<ToastContent>,
    generation: u64,
    timer: Option<Task<()>>,
}

impl ToastState {
    pub fn current(&self) -> Option<&ToastContent> {
        self.current.as_ref()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn replace(&mut self, content: ToastContent) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.current = Some(content);
        self.timer = None;
        self.generation
    }

    pub fn set_timer(&mut self, timer: Task<()>) {
        self.timer = Some(timer);
    }

    pub fn dismiss(&mut self) -> Option<ToastAction> {
        self.generation = self.generation.wrapping_add(1);
        self.timer = None;
        self.current.take().and_then(|content| content.action)
    }

    pub fn dismiss_generation(&mut self, generation: u64) -> bool {
        if self.generation != generation || self.current.is_none() {
            return false;
        }
        self.current = None;
        self.timer = None;
        true
    }
}

pub fn is_expired(elapsed: Duration) -> bool {
    elapsed >= TOAST_DURATION
}

pub fn dismissal_task<T: 'static>(
    generation: u64,
    cx: &mut Context<T>,
    dismiss: impl FnOnce(&mut T, u64, &mut Context<T>) + 'static,
) -> Task<()> {
    cx.spawn(async move |entity, cx| {
        let elapsed = TOAST_DURATION;
        cx.background_executor().timer(elapsed).await;
        if is_expired(elapsed) {
            let _ = entity.update(cx, |entity, cx| dismiss(entity, generation, cx));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(title: &str, body: &str, tone: ToastTone) -> ToastContent {
        ToastContent::new(title, body, tone, None)
    }

    #[test]
    fn toast_latest_replaces_current_and_stale_generation_cannot_dismiss() {
        let mut state = ToastState::default();
        let first = state.replace(content("First", "one", ToastTone::Info));
        let second = state.replace(content("Second", "two", ToastTone::Success));

        assert_ne!(first, second);
        assert_eq!(state.current().unwrap().title, "Second");
        assert!(!state.dismiss_generation(first));
        assert_eq!(state.current().unwrap().title, "Second");
        assert!(state.dismiss_generation(second));
        assert!(state.current().is_none());
    }

    #[test]
    fn toast_blank_body_and_accessibility_label_are_normalized() {
        let blank = ToastContent::new("Done", "  \n ", ToastTone::Success, None);
        let body = ToastContent::new("Done", "Applied", ToastTone::Success, None);

        assert_eq!(blank.body, None);
        assert_eq!(blank.accessibility_label(), "Done");
        assert_eq!(body.accessibility_label(), "Done, Applied");
    }

    #[test]
    fn toast_positions_map_the_exact_settings_catalog_and_fallback() {
        let expected = [
            ("Top Center", ToastPosition::TopCenter),
            ("Top Right", ToastPosition::TopRight),
            ("Bottom Center", ToastPosition::BottomCenter),
            ("Bottom Right", ToastPosition::BottomRight),
        ];
        assert_eq!(
            muxy_core::prefs::settings::TOAST_POSITIONS.map(|value| value),
            expected.map(|(value, _)| value)
        );
        for (value, position) in expected {
            assert_eq!(ToastPosition::from_setting(value), position);
        }
        assert_eq!(
            ToastPosition::from_setting("unknown"),
            ToastPosition::TopCenter
        );
        assert!(ToastPosition::TopRight.is_top());
        assert!(ToastPosition::BottomCenter.is_centered());
        assert!(ToastPosition::BottomRight.is_right());
    }

    #[test]
    fn toast_action_is_returned_only_after_dismissal() {
        let mut state = ToastState::default();
        state.replace(ToastContent::new(
            "Open",
            "Target",
            ToastTone::Info,
            Some(ToastAction::NavigateNotification("ID".to_owned())),
        ));

        assert_eq!(
            state.dismiss(),
            Some(ToastAction::NavigateNotification("ID".to_owned()))
        );
        assert!(state.current().is_none());
        assert_eq!(state.dismiss(), None);
    }

    #[test]
    fn feedback_bypasses_notification_toast_enablement() {
        assert!(!ToastOrigin::Notification.should_present(false));
        assert!(ToastOrigin::Feedback.should_present(false));
        assert!(ToastOrigin::Notification.should_present(true));
        assert!(ToastOrigin::Feedback.should_present(true));
    }

    #[test]
    fn toast_duration_is_exactly_three_seconds_under_controlled_clock() {
        let mut elapsed = Duration::ZERO;
        elapsed += Duration::from_millis(2_999);
        assert!(!is_expired(elapsed));
        elapsed += Duration::from_millis(1);
        assert!(is_expired(elapsed));
    }
}
