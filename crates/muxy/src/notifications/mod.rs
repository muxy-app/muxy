mod coordinator;
pub mod desktop;
pub mod navigation;
pub mod sound;

pub use coordinator::NotificationCoordinator;
pub(crate) use coordinator::{
    DeliveryInputs, NotificationOrigin, ResolvedNotificationEvent, resolve_agent_hook_notification,
    resolve_legacy_notification,
};
