//! Toast notifications through `WinRT`. Windows shows a toast only for an app
//! registered under an `AppUserModelID`, so the first start registers one for
//! the user. Windows hands a click to the COM class the registration names:
//! the running app provides it, and otherwise Windows starts the app with
//! `-Embedding` to provide it.
#![allow(
    clippy::inline_always,
    clippy::ref_as_ptr,
    reason = "The COM implement macro generates such code"
)]

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread;

use async_channel::{Receiver, Sender};
use muxy_core::capability::Capability;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{
    NotificationSetting, ToastNotification, ToastNotificationManager, ToastNotifier,
};
use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, ERROR_SUCCESS};
use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, CoRegisterClassObject, IClassFactory, IClassFactory_Impl,
    REGCLS_MULTIPLEUSE,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::Win32::UI::Notifications::{
    INotificationActivationCallback, INotificationActivationCallback_Impl,
    NOTIFICATION_USER_INPUT_DATA,
};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::{BOOL, GUID, HSTRING, IUnknown, Interface, PCWSTR, Ref, implement};

use super::Event;
use crate::tr;

pub const CAPABILITY: Capability = Capability::Supported;

/// Every toast of this app is in one group, so a tag names one toast.
const GROUP: &str = "muxy";

fn app_id() -> &'static str {
    muxy_core::release::Channel::current().bundle_identifier()
}

/// The COM class Windows hands clicks on this app's toasts to. Each release
/// channel has its own, as it has its own `AppUserModelID`.
fn activator() -> GUID {
    match muxy_core::release::Channel::current() {
        muxy_core::release::Channel::Beta => {
            GUID::from_u128(0x54ac1e3f_edc8_440c_b48b_4eb3f2f03a76)
        }
        muxy_core::release::Channel::Stable => {
            GUID::from_u128(0x25672a44_f13e_44de_9729_cd667e787c78)
        }
    }
}

/// Names this process as the app Windows groups its windows and toasts by.
/// Call it before the first window opens.
pub fn identify_process() {
    // SAFETY: the argument is a valid, NUL-terminated wide string for the call.
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(app_id())) };
}

/// The argument Windows starts the app with to hand it a click on a toast.
pub const ACTIVATION_ARGUMENT: &str = "-Embedding";

/// The registry keys, value names and values that let Windows show toasts
/// for `id`, and hand their clicks to `activator` in `executable`. An empty
/// name is the key's default value.
fn registration(
    id: &str,
    name: &str,
    activator: GUID,
    executable: &str,
) -> Vec<(String, &'static str, String)> {
    let activator = format!("{{{activator:?}}}");
    let application = format!(r"Software\Classes\AppUserModelId\{id}");
    vec![
        (application.clone(), "DisplayName", name.to_owned()),
        (application, "CustomActivator", activator.clone()),
        (
            format!(r"Software\Classes\CLSID\{activator}\LocalServer32"),
            "",
            format!("\"{executable}\""),
        ),
    ]
}

fn register() -> Result<(), String> {
    let executable = muxy_core::executable::current_path().map_err(|error| error.to_string())?;
    let values = registration(
        app_id(),
        muxy_core::release::Channel::current().app_name(),
        activator(),
        &executable.to_string_lossy(),
    );
    for (key, name, value) in values {
        let key = HSTRING::from(key);
        let data: Vec<u16> = value.encode_utf16().chain([0]).collect();
        let bytes = u32::try_from(data.len() * 2).map_err(|error| error.to_string())?;
        // SAFETY: the key, name and data are valid wide strings that outlive
        // the call, and `bytes` is the size of `data`, its NUL included.
        let result = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &key,
                &HSTRING::from(name),
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                bytes,
            )
        };
        if result != ERROR_SUCCESS {
            return Err(windows::core::Error::from(result.to_hresult()).message());
        }
    }
    Ok(())
}

/// The tag Windows knows the toast for `id` by, the same in every run, so a
/// later delivery replaces the toast and a clear removes it. Windows limits a
/// tag's length, so it is a hash (FNV-1a).
fn tag(id: &str) -> String {
    let hash = id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}")
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
    notifier: ToastNotifier,
    availability: Arc<Mutex<Capability>>,
    events: Sender<Event>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn unavailable(error: &dyn std::fmt::Display) -> Capability {
    Capability::Unavailable(
        tr!(
            "Windows refused to set up notifications: %@",
            error.to_string()
        )
        .to_string(),
    )
}

impl Service {
    pub(super) fn start() -> Result<(Self, Receiver<Event>), Capability> {
        register().map_err(|error| unavailable(&error))?;
        let (events, reported) = async_channel::bounded(32);
        let (commands, queue) = mpsc::channel();
        let (ready, started) = mpsc::channel();
        let availability = Arc::new(Mutex::new(Capability::Supported));
        let shared = availability.clone();
        thread::Builder::new()
            .name("notifications".into())
            .spawn(move || {
                // SAFETY: called once, first, on a thread of its own.
                let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
                let factory: IClassFactory = Factory(events.clone()).into();
                // SAFETY: `activator` outlives the call, and the class stays
                // registered while this thread serves, which is as long as
                // the app runs.
                if let Err(error) = unsafe {
                    CoRegisterClassObject(
                        &activator(),
                        &factory,
                        CLSCTX_LOCAL_SERVER,
                        REGCLS_MULTIPLEUSE,
                    )
                } {
                    let _ = ready.send(Err(unavailable(&error.message())));
                    return;
                }
                let notifier = match ToastNotificationManager::CreateToastNotifierWithId(
                    &HSTRING::from(app_id()),
                ) {
                    Ok(notifier) => notifier,
                    Err(error) => {
                        let _ = ready.send(Err(unavailable(&error.message())));
                        return;
                    }
                };
                let worker = Worker {
                    notifier,
                    availability: shared,
                    events,
                };
                worker.report(worker.setting());
                let _ = ready.send(Ok(()));
                worker.serve(&queue);
            })
            .map_err(|error| unavailable(&error))?;
        started.recv().map_err(|error| unavailable(&error))??;
        Ok((
            Self {
                commands,
                availability,
            },
            reported,
        ))
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
                } => {
                    if let Err(error) = self.deliver(&id, &title, &body, opens) {
                        self.report(Capability::Unavailable(
                            tr!("Windows refused a notification: %@", error.message()).to_string(),
                        ));
                    }
                }
                Command::Clear(ids) => {
                    if let Ok(history) = ToastNotificationManager::History() {
                        for id in &ids {
                            let _ = history.RemoveGroupedTagWithId(
                                &HSTRING::from(tag(id)),
                                &HSTRING::from(GROUP),
                                &HSTRING::from(app_id()),
                            );
                        }
                    }
                }
                Command::Refresh => self.report(self.setting()),
            }
        }
    }

    fn deliver(&self, id: &str, title: &str, body: &str, opens: bool) -> windows::core::Result<()> {
        let content = XmlDocument::new()?;
        content.LoadXml(&HSTRING::from(toast(opens.then_some(id), title, body)))?;
        let toast = ToastNotification::CreateToastNotification(&content)?;
        toast.SetTag(&HSTRING::from(tag(id)))?;
        toast.SetGroup(&HSTRING::from(GROUP))?;
        self.notifier.Show(&toast)
    }

    /// Whether the user lets this app notify, as Windows says now.
    fn setting(&self) -> Capability {
        match self.notifier.Setting() {
            Ok(NotificationSetting::Enabled) | Err(_) => Capability::Supported,
            Ok(_) => Capability::Unavailable(
                tr!("Notifications are turned off for Muxy in Windows Settings.").to_string(),
            ),
        }
    }

    /// Tells the app when what Windows allows has changed.
    fn report(&self, capability: Capability) {
        let mut current = lock(&self.availability);
        if *current != capability {
            current.clone_from(&capability);
            let _ = self.events.try_send(Event::Capability(capability));
        }
    }
}

/// A toast's content: its title, then its text. A click on it hands `opens`
/// to the activator.
fn toast(opens: Option<&str>, title: &str, body: &str) -> String {
    let launch = opens.map_or_else(String::new, |id| format!(r#" launch="{}""#, escaped(id)));
    format!(
        r#"<toast{launch}><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        escaped(title),
        escaped(body)
    )
}

/// Makes the activator for Windows when a toast is clicked.
#[implement(IClassFactory)]
struct Factory(Sender<Event>);

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        interface: *const GUID,
        object: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let activator: IUnknown = Activator(self.0.clone()).into();
        // SAFETY: COM passes a valid interface ID and a place for the object.
        unsafe { activator.query(interface, object) }.ok()
    }

    fn LockServer(&self, _lock: BOOL) -> windows::core::Result<()> {
        Ok(())
    }
}

/// Tells the app which of its toasts the user clicked.
#[implement(INotificationActivationCallback)]
struct Activator(Sender<Event>);

impl INotificationActivationCallback_Impl for Activator_Impl {
    fn Activate(
        &self,
        _app: &PCWSTR,
        arguments: &PCWSTR,
        _data: *const NOTIFICATION_USER_INPUT_DATA,
        _count: u32,
    ) -> windows::core::Result<()> {
        // SAFETY: Windows passes the toast's arguments as a valid string.
        let id = unsafe { arguments.to_string() }.unwrap_or_default();
        if !id.is_empty() {
            let _ = self.0.try_send(Event::Opened(id));
        }
        Ok(())
    }
}

/// `text` as XML that shows the same characters, less the control
/// characters XML can't hold.
fn escaped(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control() || matches!(character, '\t' | '\n' | '\r'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_is_registered_by_its_identifier_with_its_name() {
        let activator = GUID::from_u128(0x25672a44_f13e_44de_9729_cd667e787c78);
        let application = r"Software\Classes\AppUserModelId\com.muxy.app".to_owned();
        assert_eq!(
            registration("com.muxy.app", "Muxy", activator, r"C:\Muxy\muxy-app.exe"),
            [
                (application.clone(), "DisplayName", "Muxy".to_owned()),
                (
                    application,
                    "CustomActivator",
                    "{25672A44-F13E-44DE-9729-CD667E787C78}".to_owned()
                ),
                (
                    r"Software\Classes\CLSID\{25672A44-F13E-44DE-9729-CD667E787C78}\LocalServer32"
                        .to_owned(),
                    "",
                    r#""C:\Muxy\muxy-app.exe""#.to_owned()
                ),
            ]
        );
    }

    #[test]
    fn toast_text_is_shown_as_written() {
        assert_eq!(
            toast(None, "<b>1 & 2</b>\u{1b}", r#"say "hi""#),
            r#"<toast><visual><binding template="ToastGeneric"><text>&lt;b&gt;1 &amp; 2&lt;/b&gt;</text><text>say &quot;hi&quot;</text></binding></visual></toast>"#
        );
    }

    /// Run by hand on a desktop, and click the toast within a minute.
    #[test]
    #[ignore = "shows a toast and waits for a click on it"]
    #[allow(clippy::expect_used, clippy::panic, reason = "Manual desktop check")]
    fn a_clicked_toast_reaches_the_app() {
        let (service, events) = Service::start().expect("notifications");
        service.deliver("check", "Muxy", "Click this notification");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while std::time::Instant::now() < deadline {
            match events.try_recv() {
                Ok(Event::Opened(id)) => return assert_eq!(id, "check"),
                Ok(Event::Capability(capability)) => panic!("{capability:?}"),
                Err(_) => thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
        panic!("the toast was not clicked");
    }

    #[test]
    fn only_a_toast_that_opens_something_hands_its_identifier_to_a_click() {
        assert!(
            toast(Some(r#"remote:"7""#), "t", "b")
                .starts_with(r#"<toast launch="remote:&quot;7&quot;">"#)
        );
        assert!(toast(None, "t", "b").starts_with("<toast><visual>"));
    }

    #[test]
    fn an_alert_keeps_its_tag_across_runs_so_it_replaces_and_clears_its_toast() {
        assert_eq!(tag("7"), tag("7"));
        assert_ne!(tag("7"), tag("remote:7"));
        assert_eq!(tag(""), "cbf29ce484222325", "fixed, not seeded per run");
        assert!(tag(&"x".repeat(500)).len() <= 64, "within Windows' limit");
    }
}
