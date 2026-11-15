use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread;

use async_channel::{Receiver, Sender};
use muxy_core::capability::Capability;
use zbus::blocking::{Connection, Proxy, proxy::SignalIterator};
use zbus::zvariant::Value;

use super::Event;
use crate::tr;

pub const CAPABILITY: Capability = Capability::Supported;

const SERVICE: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const OPEN: &str = "default";

#[derive(Debug, Default)]
struct Shown(HashMap<String, u32>);

impl Shown {
    fn replaces(&self, id: &str) -> u32 {
        self.0.get(id).copied().unwrap_or(0)
    }

    fn record(&mut self, id: String, number: u32) {
        self.0.insert(id, number);
    }

    fn take(&mut self, ids: &[String]) -> Vec<u32> {
        ids.iter().filter_map(|id| self.0.remove(id)).collect()
    }

    fn identifier(&self, number: u32) -> Option<String> {
        self.0
            .iter()
            .find_map(|(id, shown)| (*shown == number).then(|| id.clone()))
    }

    fn closed(&mut self, number: u32) {
        self.0.retain(|_, shown| *shown != number);
    }
}

enum Command {
    Deliver {
        id: String,
        title: String,
        body: String,
        opens: bool,
    },
    Clear(Vec<String>),
    Refresh,
}

#[derive(Debug)]
pub(super) struct Service {
    commands: mpsc::Sender<Command>,
    availability: Arc<Mutex<Capability>>,
}

struct Worker {
    connection: Connection,
    proxy: Proxy<'static>,
    shown: Arc<Mutex<Shown>>,
    availability: Arc<Mutex<Capability>>,
    events: Sender<Event>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn availability(connection: &Connection) -> Capability {
    if crate::dbus::offers(connection, SERVICE) {
        Capability::Supported
    } else {
        Capability::Unavailable(
            tr!("No notification service is running on this desktop.").to_string(),
        )
    }
}

impl Service {
    pub(super) fn start() -> Result<(Self, Receiver<Event>), Capability> {
        let unreachable = |error: &dyn std::fmt::Display| {
            Capability::Unavailable(
                tr!(
                    "Desktop notifications need the session's D-Bus: %@",
                    error.to_string()
                )
                .to_string(),
            )
        };
        let connection = crate::dbus::session().map_err(|error| unreachable(&error))?;
        let proxy =
            Proxy::new(&connection, SERVICE, PATH, SERVICE).map_err(|error| unreachable(&error))?;
        let signals = proxy
            .receive_all_signals()
            .map_err(|error| unreachable(&error))?;
        let (events, reported) = async_channel::bounded(32);
        let (commands, queue) = mpsc::channel();
        let worker = Worker {
            availability: Arc::new(Mutex::new(availability(&connection))),
            connection,
            proxy,
            shown: Arc::default(),
            events,
        };
        let service = Self {
            commands,
            availability: worker.availability.clone(),
        };
        let (shown, opened) = (worker.shown.clone(), worker.events.clone());
        let started = thread::Builder::new()
            .name("notification-signals".into())
            .spawn(move || follow(signals, &shown, &opened))
            .and_then(|_| {
                thread::Builder::new()
                    .name("notifications".into())
                    .spawn(move || worker.serve(&queue))
            });
        match started {
            Ok(_) => Ok((service, reported)),
            Err(error) => Err(unreachable(&error)),
        }
    }

    pub(super) fn capability(&self) -> Capability {
        lock(&self.availability).clone()
    }

    pub(super) fn refresh(&self) {
        let _ = self.commands.send(Command::Refresh);
    }

    pub(super) fn deliver(&self, id: &str, title: &str, body: &str) {
        self.show(id, title, body, true);
    }

    pub(super) fn inform(&self, id: &str, title: &str, body: &str) {
        self.show(id, title, body, false);
    }

    fn show(&self, id: &str, title: &str, body: &str, opens: bool) {
        let _ = self.commands.send(Command::Deliver {
            id: id.to_owned(),
            title: title.to_owned(),
            body: body.to_owned(),
            opens,
        });
    }

    pub(super) fn clear(&self, ids: &[String]) {
        let _ = self.commands.send(Command::Clear(ids.to_vec()));
    }
}

impl Worker {
    fn serve(self, queue: &mpsc::Receiver<Command>) {
        for command in queue {
            match command {
                Command::Deliver {
                    id,
                    title,
                    body,
                    opens,
                } => self.deliver(id, &title, &body, opens),
                Command::Clear(ids) => {
                    let numbers = lock(&self.shown).take(&ids);
                    for number in numbers {
                        let _ = self.proxy.call::<_, _, ()>("CloseNotification", &(number,));
                    }
                }
                Command::Refresh => self.report(availability(&self.connection)),
            }
        }
    }

    fn reads_markup(&self) -> bool {
        self.proxy
            .call::<_, _, Vec<String>>("GetCapabilities", &())
            .is_ok_and(|features| features.iter().any(|feature| feature == "body-markup"))
    }

    fn deliver(&self, id: String, title: &str, body: &str, opens: bool) {
        let name = muxy_core::release::Channel::current().app_name();
        let open = tr!("Show");
        let body = if self.reads_markup() {
            escaped(body)
        } else {
            body.to_owned()
        };
        let actions = if opens {
            vec![OPEN, open.as_ref()]
        } else {
            Vec::new()
        };
        let mut shown = lock(&self.shown);
        let sent = self.proxy.call::<_, _, u32>(
            "Notify",
            &(
                name,
                shown.replaces(&id),
                "",
                title,
                body,
                actions,
                HashMap::<&str, Value<'_>>::new(),
                -1_i32,
            ),
        );
        match sent {
            Ok(number) => shown.record(id, number),
            Err(error) => {
                drop(shown);
                self.report(Capability::Unavailable(
                    tr!("The desktop refused a notification: %@", error.to_string()).to_string(),
                ));
            }
        }
    }

    fn report(&self, capability: Capability) {
        let mut current = lock(&self.availability);
        if *current != capability {
            current.clone_from(&capability);
            let _ = self.events.try_send(Event::Capability(capability));
        }
    }
}

fn follow(signals: SignalIterator<'static>, shown: &Mutex<Shown>, events: &Sender<Event>) {
    for signal in signals {
        let header = signal.header();
        let body = signal.body();
        match header.member().map(zbus::names::MemberName::as_str) {
            Some("ActionInvoked") => {
                if let Ok((number, action)) = body.deserialize::<(u32, &str)>()
                    && action == OPEN
                    && let Some(id) = lock(shown).identifier(number)
                {
                    let _ = events.try_send(Event::Opened(id));
                }
            }
            Some("NotificationClosed") => {
                if let Ok((number, _reason)) = body.deserialize::<(u32, u32)>() {
                    lock(shown).closed(number);
                }
            }
            _ => {}
        }
    }
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_shown_as_written_where_the_desktop_reads_markup() {
        assert_eq!(
            escaped("<b>1 & 2</b> &amp;"),
            "&lt;b&gt;1 &amp; 2&lt;/b&gt; &amp;amp;"
        );
    }

    #[test]
    fn a_repeated_alert_replaces_its_notification_and_a_cleared_one_is_closed_once() {
        let mut shown = Shown::default();
        assert_eq!(shown.replaces("7"), 0);
        shown.record("7".into(), 41);
        shown.record("remote:7".into(), 42);
        assert_eq!(shown.replaces("7"), 41, "a second delivery replaces");
        shown.record("7".into(), 41);
        assert_eq!(shown.take(&["7".into(), "unknown".into()]), vec![41]);
        assert!(shown.take(&["7".into()]).is_empty(), "closed only once");
        assert_eq!(shown.identifier(42).as_deref(), Some("remote:7"));
    }

    #[test]
    fn clicks_reach_only_notifications_this_app_still_shows() {
        let mut shown = Shown::default();
        shown.record("7".into(), 41);
        assert_eq!(shown.identifier(41).as_deref(), Some("7"));
        assert_eq!(shown.identifier(40), None, "another app's notification");
        shown.closed(41);
        assert_eq!(shown.identifier(41), None, "dismissed before the click");
        assert!(shown.take(&["7".into()]).is_empty());
    }
}
