use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::Poll;

use ashpd::WindowIdentifier;
use gpui::{App, AsyncApp};
use zbus::export::ordered_stream::OrderedStreamExt;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::tr;

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const REQUEST: &str = "org.freedesktop.portal.Request";

#[derive(Debug)]
pub struct FolderPicker {
    _open: async_channel::Sender<()>,
}

pub fn choose_folder(
    cx: &mut App,
    message: &str,
    directory: &Path,
    on_complete: impl FnOnce(io::Result<Option<PathBuf>>) + 'static,
) -> io::Result<FolderPicker> {
    choose_folder_with(cx, message, "", directory, on_complete)
}

pub fn choose_folder_with(
    cx: &mut App,
    message: &str,
    prompt: &str,
    directory: &Path,
    on_complete: impl FnOnce(io::Result<Option<PathBuf>>) + 'static,
) -> io::Result<FolderPicker> {
    Ok(open(
        cx,
        Chooser::new(message, prompt, directory, false),
        on_complete,
    ))
}

pub fn choose_file(
    cx: &mut App,
    message: &str,
    directory: &Path,
    on_complete: impl FnOnce(io::Result<Option<PathBuf>>) + 'static,
) -> io::Result<FolderPicker> {
    Ok(open(
        cx,
        Chooser::new(message, "", directory, true),
        on_complete,
    ))
}

fn open(
    cx: &mut App,
    chooser: Chooser,
    on_complete: impl FnOnce(io::Result<Option<PathBuf>>) + 'static,
) -> FolderPicker {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let token = format!(
        "muxy_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let (open, dropped) = async_channel::bounded(1);
    cx.spawn(async move |cx| {
        let ask = chooser.ask(&token, cx);
        until_dropped(ask, dropped, on_complete, close(&token)).await;
    })
    .detach();
    FolderPicker { _open: open }
}

async fn until_dropped<A>(
    ask: impl Future<Output = A>,
    dropped: async_channel::Receiver<()>,
    on_complete: impl FnOnce(A),
    close: impl Future<Output = ()>,
) {
    let answer = {
        let mut ask = std::pin::pin!(async { Some(ask.await) });
        let mut dropped = std::pin::pin!(async {
            let _ = dropped.recv().await;
            None
        });
        std::future::poll_fn(|cx| match ask.as_mut().poll(cx) {
            Poll::Pending => dropped.as_mut().poll(cx),
            ready @ Poll::Ready(_) => ready,
        })
        .await
    };
    match answer {
        Some(answer) => on_complete(answer),
        None => close.await,
    }
}

async fn close(token: &str) {
    let Ok(connection) = crate::dbus::session() else {
        return;
    };
    let connection = connection.inner();
    if let Some(path) = request_path(connection, token) {
        let _ = connection
            .call_method(Some(PORTAL), path.as_str(), Some(REQUEST), "Close", &())
            .await;
    }
}

fn request_path(connection: &zbus::Connection, token: &str) -> Option<String> {
    Some(request_path_for(connection.unique_name()?.as_str(), token))
}

fn request_path_for(unique_name: &str, token: &str) -> String {
    let sender = unique_name.trim_start_matches(':').replace('.', "_");
    format!("{PORTAL_PATH}/request/{sender}/{token}")
}

struct Chooser {
    title: String,
    accept: String,
    directory: PathBuf,
    files: bool,
}

impl Chooser {
    fn new(title: &str, accept: &str, directory: &Path, files: bool) -> Self {
        Self {
            title: title.to_owned(),
            accept: accept.to_owned(),
            directory: directory.to_owned(),
            files,
        }
    }

    async fn ask(&self, token: &str, cx: &mut AsyncApp) -> io::Result<Option<PathBuf>> {
        let parent = parent(cx).await;
        let window = parent.as_ref().map(ToString::to_string).unwrap_or_default();
        let connection = crate::dbus::session().map_err(explain)?;
        let connection = connection.inner();
        let path =
            request_path(connection, token).ok_or_else(|| explain(zbus::Error::Unsupported))?;
        let request = zbus::Proxy::new(connection, PORTAL, path, REQUEST)
            .await
            .map_err(explain)?;
        let mut responses = request.receive_signal("Response").await.map_err(explain)?;
        let portal = zbus::Proxy::new(
            connection,
            PORTAL,
            PORTAL_PATH,
            "org.freedesktop.portal.FileChooser",
        )
        .await
        .map_err(explain)?;
        portal
            .call::<_, _, OwnedObjectPath>(
                "OpenFile",
                &(window.as_str(), self.title.as_str(), self.options(token)),
            )
            .await
            .map_err(explain)?;
        let Some(response) = responses.next().await else {
            return Ok(None);
        };
        let (code, results) = response
            .body()
            .deserialize::<(u32, HashMap<String, OwnedValue>)>()
            .map_err(explain)?;
        Ok(selected(code, results))
    }

    fn options<'a>(&'a self, token: &'a str) -> HashMap<&'static str, Value<'a>> {
        let mut options = HashMap::from([
            ("handle_token", Value::from(token)),
            ("modal", Value::from(true)),
            ("multiple", Value::from(false)),
            ("directory", Value::from(!self.files)),
        ]);
        if !self.accept.is_empty() {
            options.insert("accept_label", Value::from(self.accept.as_str()));
        }
        if let Some(folder) = current_folder(&self.directory) {
            options.insert("current_folder", Value::from(folder));
        }
        options
    }
}

fn current_folder(directory: &Path) -> Option<Vec<u8>> {
    let existing = directory.ancestors().find(|folder| folder.is_dir())?;
    let bytes = existing.as_os_str().as_bytes();
    (existing.is_absolute() && !bytes.contains(&0)).then(|| [bytes, &[0]].concat())
}

fn selected(code: u32, mut results: HashMap<String, OwnedValue>) -> Option<PathBuf> {
    if code != 0 {
        return None;
    }
    let uris = Vec::<String>::try_from(results.remove("uris")?).ok()?;
    let [uri] = uris.as_slice() else {
        return None;
    };
    let url = ashpd::url::Url::parse(uri).ok()?;
    (url.scheme() == "file")
        .then(|| url.to_file_path().ok())
        .flatten()
}

async fn parent(cx: &mut AsyncApp) -> Option<WindowIdentifier> {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
    let window = cx.update(|cx| cx.active_window()).ok()??;
    let (window, display) = window
        .update(cx, |_, window, _| {
            let handle = HasWindowHandle::window_handle(window).ok()?.as_raw();
            let display = HasDisplayHandle::display_handle(window)
                .ok()
                .map(|display| display.as_raw());
            Some((handle, display))
        })
        .ok()??;
    WindowIdentifier::from_raw_handle(&window, display.as_ref()).await
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Maps the errors of bus calls"
)]
fn explain(error: zbus::Error) -> io::Error {
    let name = match &error {
        zbus::Error::MethodError(name, _, _) => name.as_str().to_owned(),
        zbus::Error::FDO(error) => zbus::DBusError::name(&**error).as_str().to_owned(),
        _ => String::new(),
    };
    let missing = [
        "org.freedesktop.DBus.Error.ServiceUnknown",
        "org.freedesktop.DBus.Error.NameHasNoOwner",
        "org.freedesktop.DBus.Error.UnknownInterface",
        "org.freedesktop.DBus.Error.UnknownMethod",
    ]
    .contains(&name.as_str());
    io::Error::other(if missing {
        tr!(
            "This desktop has no file chooser service. Install xdg-desktop-portal and its backend for your desktop."
        )
        .to_string()
    } else {
        tr!("Could not open the file chooser: %@", error.to_string()).to_string()
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "Regression fixture setup")]
mod tests {
    use std::cell::Cell;
    use std::ffi::OsStr;
    use std::rc::Rc;

    use super::*;

    fn uris(uris: &[&str]) -> HashMap<String, OwnedValue> {
        let uris: Vec<String> = uris.iter().map(|uri| (*uri).to_owned()).collect();
        HashMap::from([(
            "uris".to_owned(),
            OwnedValue::try_from(Value::from(uris)).expect("value"),
        )])
    }

    #[test]
    fn a_request_is_found_where_the_portal_puts_it() {
        assert_eq!(
            request_path_for(":1.42", "muxy_7_0"),
            "/org/freedesktop/portal/desktop/request/1_42/muxy_7_0"
        );
    }

    #[test]
    fn the_dialog_starts_in_the_nearest_existing_folder_with_its_exact_bytes() {
        let root = std::env::temp_dir().join(format!("muxy-picker-{}", std::process::id()));
        let odd = root.join(OsStr::from_bytes(b"caf\xc3\xa9 \xff"));
        std::fs::create_dir_all(&odd).expect("folder");
        let expected = [odd.as_os_str().as_bytes(), &[0]].concat();
        assert_eq!(current_folder(&odd), Some(expected.clone()));
        assert_eq!(current_folder(&odd.join("gone/deeper")), Some(expected));
        assert_eq!(current_folder(Path::new("relative/gone")), None);
        std::fs::remove_dir_all(&root).expect("cleanup");

        let chooser = Chooser::new("Select a project folder", "", &odd, false);
        let options = chooser.options("muxy_7_0");
        assert_eq!(options["directory"], Value::from(true));
        assert_eq!(options["handle_token"], Value::from("muxy_7_0"));
        assert!(!options.contains_key("accept_label"));
        let chooser = Chooser::new("Choose a private key", "Use Key", &odd, true);
        let options = chooser.options("muxy_7_1");
        assert_eq!(options["directory"], Value::from(false));
        assert_eq!(options["accept_label"], Value::from("Use Key"));
    }

    #[test]
    fn only_one_confirmed_local_file_is_a_choice() {
        assert_eq!(
            selected(0, uris(&["file:///home/me/My%20Project/caf%C3%A9%FF"])),
            Some(PathBuf::from(OsStr::from_bytes(
                b"/home/me/My Project/caf\xc3\xa9\xff"
            )))
        );
        assert_eq!(selected(1, uris(&["file:///home/me"])), None, "cancelled");
        assert_eq!(selected(2, uris(&["file:///home/me"])), None, "ended");
        assert_eq!(selected(0, uris(&["file:///a", "file:///b"])), None);
        assert_eq!(selected(0, uris(&["sftp://host/home/me"])), None);
        assert_eq!(selected(0, uris(&["trash:///gone"])), None);
        assert_eq!(selected(0, HashMap::new()), None);
    }

    #[gpui::test]
    fn a_dropped_picker_closes_its_dialog_and_never_answers(cx: &mut gpui::TestAppContext) {
        let (open, dropped) = async_channel::bounded::<()>(1);
        let answered = Rc::new(Cell::new(false));
        let closed = Rc::new(Cell::new(false));
        let (answer, close) = (answered.clone(), closed.clone());
        cx.foreground_executor()
            .spawn(until_dropped(
                std::future::pending::<io::Result<Option<PathBuf>>>(),
                dropped,
                move |_| answer.set(true),
                async move { close.set(true) },
            ))
            .detach();
        cx.run_until_parked();
        assert!(!closed.get(), "an open picker keeps waiting");
        drop(open);
        cx.run_until_parked();
        assert!(closed.get());
        assert!(!answered.get());
    }

    #[gpui::test]
    fn an_answer_is_delivered_once_and_closes_nothing(cx: &mut gpui::TestAppContext) {
        let (open, dropped) = async_channel::bounded::<()>(1);
        let answers = Rc::new(Cell::new(0));
        let closed = Rc::new(Cell::new(false));
        let (answer, close) = (answers.clone(), closed.clone());
        cx.foreground_executor()
            .spawn(until_dropped(
                async { Err::<Option<PathBuf>, _>(io::Error::other("no portal")) },
                dropped,
                move |result| {
                    assert!(result.is_err(), "a failure is not a cancel");
                    answer.set(answer.get() + 1);
                },
                async move { close.set(true) },
            ))
            .detach();
        cx.run_until_parked();
        drop(open);
        cx.run_until_parked();
        assert_eq!(answers.get(), 1);
        assert!(!closed.get());
    }
}
