#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(target_os = "macos")]
pub use macos::PlatformDesktopService;
#[cfg(not(target_os = "macos"))]
pub use unsupported::PlatformDesktopService;

use muxy_core::notifications::canonical_uuid;

pub const NATIVE_RESPONSE_CAPACITY: usize = 32;
pub const DEFAULT_ACTION_IDENTIFIER: &str = "com.apple.UNNotificationDefaultActionIdentifier";
pub const DISMISS_ACTION_IDENTIFIER: &str = "com.apple.UNNotificationDismissActionIdentifier";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationStatus {
    NotDetermined,
    Denied,
    Authorized,
    Provisional,
    Ephemeral,
    Unavailable,
}

impl AuthorizationStatus {
    pub fn allows_scheduling(self) -> bool {
        matches!(self, Self::Authorized | Self::Provisional | Self::Ephemeral)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationResult {
    Allowed,
    Denied,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopRequest {
    pub identifier: String,
    pub title: String,
    pub body: String,
    pub notification_id: String,
}

impl DesktopRequest {
    pub fn new(
        notification_id: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Option<Self> {
        let notification_id = canonical_uuid(&notification_id.into())?;
        Some(Self {
            identifier: notification_id.clone(),
            title: title.into(),
            body: body.into(),
            notification_id,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseAction<'a> {
    Default,
    Dismiss,
    Other(&'a str),
}

pub fn response_action(identifier: &str) -> ResponseAction<'_> {
    match identifier {
        DEFAULT_ACTION_IDENTIFIER => ResponseAction::Default,
        DISMISS_ACTION_IDENTIFIER => ResponseAction::Dismiss,
        other => ResponseAction::Other(other),
    }
}

pub fn complete_response(
    action: ResponseAction<'_>,
    notification_id: Option<&str>,
    sender: &async_channel::Sender<String>,
    completion: impl FnOnce(),
) {
    if action == ResponseAction::Default
        && let Some(notification_id) = notification_id.and_then(canonical_uuid)
        && sender.try_send(notification_id).is_err()
    {
        log::warn!("native notification response queue unavailable");
    }
    completion();
}

pub struct DesktopNotificationService {
    platform: PlatformDesktopService,
}

impl DesktopNotificationService {
    pub fn prepare() -> (Self, async_channel::Receiver<String>) {
        let (sender, receiver) = async_channel::bounded(NATIVE_RESPONSE_CAPACITY);
        (
            Self {
                platform: PlatformDesktopService::prepare(sender),
            },
            receiver,
        )
    }

    pub fn query_authorization(&self) -> async_channel::Receiver<AuthorizationStatus> {
        self.platform.query_authorization()
    }

    pub fn request_authorization(&self) -> async_channel::Receiver<AuthorizationResult> {
        self.platform.request_authorization()
    }

    pub fn schedule(&self, request: DesktopRequest) {
        self.platform.schedule(request);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    const ID: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
    const UPPER_ID: &str = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE";

    #[test]
    fn notifications_desktop_request_payload_is_exact() {
        let request = DesktopRequest::new(ID, "Title", "Body").expect("request");
        assert_eq!(request.identifier, UPPER_ID);
        assert_eq!(request.notification_id, UPPER_ID);
        assert_eq!(request.title, "Title");
        assert_eq!(request.body, "Body");
        assert!(DesktopRequest::new("bad", "Title", "Body").is_none());
    }

    #[test]
    fn notifications_desktop_authorization_policy_is_exact() {
        assert!(!AuthorizationStatus::NotDetermined.allows_scheduling());
        assert!(!AuthorizationStatus::Denied.allows_scheduling());
        assert!(AuthorizationStatus::Authorized.allows_scheduling());
        assert!(AuthorizationStatus::Provisional.allows_scheduling());
        assert!(AuthorizationStatus::Ephemeral.allows_scheduling());
        assert!(!AuthorizationStatus::Unavailable.allows_scheduling());
    }

    #[test]
    fn notifications_desktop_response_filters_actions_and_completes_once() {
        let (sender, receiver) = async_channel::bounded(NATIVE_RESPONSE_CAPACITY);
        for (identifier, id, expected) in [
            (DEFAULT_ACTION_IDENTIFIER, Some(ID), 1),
            (DISMISS_ACTION_IDENTIFIER, Some(ID), 0),
            ("future", Some(ID), 0),
            (DEFAULT_ACTION_IDENTIFIER, Some("bad"), 0),
            (DEFAULT_ACTION_IDENTIFIER, None, 0),
        ] {
            let completions = Rc::new(Cell::new(0));
            let observed = completions.clone();
            complete_response(response_action(identifier), id, &sender, move || {
                observed.set(observed.get() + 1);
            });
            assert_eq!(completions.get(), 1);
            assert_eq!(receiver.len(), expected);
            while receiver.try_recv().is_ok() {}
        }
    }

    #[test]
    fn notifications_desktop_response_queue_drops_newest_when_full_or_closed() {
        let (sender, receiver) = async_channel::bounded(NATIVE_RESPONSE_CAPACITY);
        for index in 0..NATIVE_RESPONSE_CAPACITY {
            sender.try_send(format!("{index}")).expect("prefill");
        }
        let completions = Rc::new(Cell::new(0));
        let observed = completions.clone();
        complete_response(ResponseAction::Default, Some(ID), &sender, move || {
            observed.set(observed.get() + 1);
        });
        assert_eq!(completions.get(), 1);
        assert_eq!(receiver.len(), NATIVE_RESPONSE_CAPACITY);
        assert_eq!(receiver.try_recv().expect("oldest"), "0");

        receiver.close();
        let completions = Rc::new(Cell::new(0));
        let observed = completions.clone();
        complete_response(ResponseAction::Default, Some(ID), &sender, move || {
            observed.set(observed.get() + 1);
        });
        assert_eq!(completions.get(), 1);
    }

    #[test]
    fn notifications_desktop_uuid_validation_uses_core_canonicalization() {
        assert_eq!(canonical_uuid(ID).as_deref(), Some(UPPER_ID));
    }
}
