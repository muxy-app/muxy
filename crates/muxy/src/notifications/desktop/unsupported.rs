use super::{AuthorizationResult, AuthorizationStatus, DesktopRequest};

pub struct PlatformDesktopService;

impl PlatformDesktopService {
    pub fn prepare(_sender: async_channel::Sender<String>) -> Self {
        Self
    }

    pub fn query_authorization(&self) -> async_channel::Receiver<AuthorizationStatus> {
        let (sender, receiver) = async_channel::bounded(1);
        let _ = sender.try_send(AuthorizationStatus::Unavailable);
        receiver
    }

    pub fn request_authorization(&self) -> async_channel::Receiver<AuthorizationResult> {
        let (sender, receiver) = async_channel::bounded(1);
        let _ = sender.try_send(AuthorizationResult::Unavailable);
        receiver
    }

    pub fn schedule(&self, _request: DesktopRequest) {}
}
