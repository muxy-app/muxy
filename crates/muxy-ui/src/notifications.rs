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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    Opened(String),
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
    pub fn start() -> (Self, Receiver<Event>) {
        match Service::start() {
            Ok((service, events)) => (Self(Backend::Native(service)), events),
            Err(capability) => {
                let (_, events) = async_channel::bounded(1);
                (Self(Backend::Absent(capability)), events)
            }
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn absent() -> (Self, Receiver<Event>) {
        let (_, events) = async_channel::bounded(1);
        (Self(Backend::Absent(Capability::Unsupported)), events)
    }

    pub fn capability(&self) -> Capability {
        match &self.0 {
            Backend::Native(service) => service.capability(),
            #[cfg(any(test, feature = "test-support"))]
            Backend::Simulated(_) => Capability::Supported,
            Backend::Absent(capability) => capability.clone(),
        }
    }

    pub fn refresh(&self) {
        if let Backend::Native(service) = &self.0 {
            service.refresh();
        }
    }

    pub fn deliver(&self, id: &str, title: &str, body: &str) {
        match &self.0 {
            Backend::Native(service) => service.deliver(id, title, body),
            #[cfg(any(test, feature = "test-support"))]
            Backend::Simulated(simulated) => simulated.shown().push(id.to_owned()),
            Backend::Absent(_) => {}
        }
    }

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

    pub fn visible(&self) -> Vec<String> {
        self.shown().clone()
    }

    pub fn open(&self, id: &str) {
        let _ = self.events.try_send(Event::Opened(id.to_owned()));
    }

    pub fn report(&self, capability: Capability) {
        let _ = self.events.try_send(Event::Capability(capability));
    }
}
