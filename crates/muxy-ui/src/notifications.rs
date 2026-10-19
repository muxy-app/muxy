//! Native delivery only; notification history and policy belong to callers.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(any(test, feature = "test-support"))]
use std::sync::{Arc, Mutex, PoisonError};

use async_channel::Receiver;
#[cfg(any(test, feature = "test-support"))]
use async_channel::Sender;
use muxy_core::capability::Capability;

#[cfg(windows)]
use self::windows::Service;
#[cfg(windows)]
pub use self::windows::{ACTIVATION_ARGUMENT, CAPABILITY, identify_process};
#[cfg(target_os = "linux")]
pub use linux::CAPABILITY;
#[cfg(target_os = "linux")]
use linux::Service;
#[cfg(target_os = "macos")]
pub use macos::CAPABILITY;
#[cfg(target_os = "macos")]
use macos::Service;

/// What a notification service reports while it runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    /// The user opened the notification with this identifier.
    Opened(String),
    /// What the system lets the service do changed.
    Capability(Capability),
}

#[derive(Debug)]
enum Backend {
    Native(Service),
    #[cfg(any(test, feature = "test-support"))]
    Simulated(SimulatedNotifications),
    Absent(Capability),
}

#[derive(Debug)]
pub struct Notifications(Backend);

impl Notifications {
    /// Starts the platform's notification service, and the events it reports.
    pub fn start() -> (Self, Receiver<Event>) {
        match Service::start() {
            Ok((service, events)) => (Self(Backend::Native(service)), events),
            Err(capability) => {
                let (_, events) = async_channel::bounded(1);
                (Self(Backend::Absent(capability)), events)
            }
        }
    }

    /// A service with no backend, for tests that must not reach the desktop.
    #[cfg(any(test, feature = "test-support"))]
    pub fn absent() -> (Self, Receiver<Event>) {
        let (_, events) = async_channel::bounded(1);
        (Self(Backend::Absent(Capability::Unsupported)), events)
    }

    /// Whether a notification delivered now would reach the user.
    pub fn capability(&self) -> Capability {
        match &self.0 {
            Backend::Native(service) => service.capability(),
            #[cfg(any(test, feature = "test-support"))]
            Backend::Simulated(_) => Capability::Supported,
            Backend::Absent(capability) => capability.clone(),
        }
    }

    /// Asks the service what the system lets it do now. A change arrives as
    /// an [`Event::Capability`].
    pub fn refresh(&self) {
        if let Backend::Native(service) = &self.0 {
            service.refresh();
        }
    }

    /// Shows a notification that opens something in the app: an
    /// [`Event::Opened`] follows when the user opens it.
    pub fn deliver(&self, id: &str, title: &str, body: &str) {
        match &self.0 {
            Backend::Native(service) => service.deliver(id, title, body),
            #[cfg(any(test, feature = "test-support"))]
            Backend::Simulated(simulated) => simulated.shown().push(id.to_owned()),
            Backend::Absent(_) => {}
        }
    }

    /// Shows a notification that only informs, with nothing for the user to open.
    pub fn inform(&self, id: &str, title: &str, body: &str) {
        match &self.0 {
            Backend::Native(service) => service.inform(id, title, body),
            #[cfg(any(test, feature = "test-support"))]
            Backend::Simulated(simulated) => simulated.shown().push(id.to_owned()),
            Backend::Absent(_) => {}
        }
    }

    pub fn clear(&self, ids: &[String]) {
        match &self.0 {
            Backend::Native(service) => service.clear(ids),
            #[cfg(any(test, feature = "test-support"))]
            Backend::Simulated(simulated) => simulated.shown().retain(|id| !ids.contains(id)),
            Backend::Absent(_) => {}
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Debug)]
pub struct SimulatedNotifications {
    shown: Arc<Mutex<Vec<String>>>,
    events: Sender<Event>,
}

#[cfg(any(test, feature = "test-support"))]
impl SimulatedNotifications {
    pub fn start() -> (Notifications, Receiver<Event>, Self) {
        let (sender, events) = async_channel::bounded(32);
        let simulated = Self {
            shown: Arc::default(),
            events: sender,
        };
        (
            Notifications(Backend::Simulated(simulated.clone())),
            events,
            simulated,
        )
    }

    fn shown(&self) -> std::sync::MutexGuard<'_, Vec<String>> {
        self.shown.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Identifiers delivered and not yet cleared, in delivery order.
    pub fn visible(&self) -> Vec<String> {
        self.shown().clone()
    }

    /// Acts as the user opening the notification `id`.
    pub fn open(&self, id: &str) {
        let _ = self.events.try_send(Event::Opened(id.to_owned()));
    }

    /// Acts as the system changing what the service may do.
    pub fn report(&self, capability: Capability) {
        let _ = self.events.try_send(Event::Capability(capability));
    }
}
