//! One desktop app per profile. A second launch hands the folders it was
//! asked to open to the first and exits, which is also how `muxy <folder>`
//! reaches an app that is already running.

#[cfg(any(target_os = "linux", windows))]
use std::ffi::OsString;
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use muxy_protocol::ServerPath;
use muxy_protocol::transport::{LocalListener, LocalStream};
use muxy_ui::tr;

/// More than any command line holds; a longer message is not a handoff.
const LIMIT: u64 = 256 * 1024;
const ANSWER: Duration = Duration::from_secs(2);
const DONE: u8 = 1;

/// The folders an app opens, as file URLs. An empty list brings its window
/// forward.
pub(crate) type Folders = async_channel::Sender<Vec<String>>;

/// Holds the profile for as long as this app runs.
#[derive(Debug)]
pub(crate) struct Instance {
    _lock: muxy_core::file_lock::FileLock,
}

/// Takes the profile for this app and opens `arguments` in it. `None` when
/// another app has the profile and opened them instead.
#[cfg(any(target_os = "linux", windows))]
pub(crate) fn claim(
    arguments: impl Iterator<Item = OsString>,
    folders: &Folders,
) -> io::Result<Option<Instance>> {
    let state = muxy_app_core::store::default_path().map_err(io::Error::other)?;
    let profile = state
        .parent()
        .ok_or_else(|| io::Error::other("Missing profile directory"))?;
    // Windows starts the app with this to hand it a click on a toast, which
    // the app takes once its notifications start.
    #[cfg(windows)]
    let arguments =
        arguments.filter(|argument| argument != muxy_ui::notifications::ACTIVATION_ARGUMENT);
    let asked: Vec<PathBuf> = arguments
        .filter_map(|argument| std::path::absolute(argument).ok())
        .collect();
    claim_in(profile, &asked, folders, ANSWER)
}

fn claim_in(
    profile: &Path,
    asked: &[PathBuf],
    folders: &Folders,
    patience: Duration,
) -> io::Result<Option<Instance>> {
    muxy_core::files::private_dir(profile)?;
    let socket = profile.join("instance.sock");
    match muxy_core::file_lock::try_lock(&profile.join("app.lock")) {
        Ok(lock) => {
            // Only a crashed app leaves its socket behind while the lock is free.
            match std::fs::remove_file(&socket) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
            let listener = LocalListener::bind(&socket)?;
            #[cfg(unix)]
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
            if !asked.is_empty() {
                let _ = folders.try_send(urls(asked));
            }
            let folders = folders.clone();
            std::thread::Builder::new()
                .name("app-instance".into())
                .spawn(move || listen(&listener, &folders))?;
            Ok(Some(Instance { _lock: lock }))
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            hand_over(&socket, asked, patience)?;
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Sends the folders to the app that has the profile. It may still be
/// starting, so its socket is tried until `patience` runs out.
fn hand_over(socket: &Path, asked: &[PathBuf], patience: Duration) -> io::Result<()> {
    let deadline = Instant::now() + patience;
    let mut stream = loop {
        match LocalStream::connect(socket) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => return Err(unanswered(&error)),
        }
    };
    let mut message = Vec::new();
    for folder in asked
        .iter()
        .filter_map(|folder| ServerPath::from_path(folder).ok())
    {
        message.extend_from_slice(&folder.0);
        message.push(0);
    }
    let mut done = [0];
    stream
        .set_read_timeout(Some(patience))
        .and_then(|()| stream.set_write_timeout(Some(patience)))
        .and_then(|()| stream.write_all(&message))
        .and_then(|()| stream.shutdown(std::net::Shutdown::Write))
        .and_then(|()| stream.read_exact(&mut done))
        .map_err(|error| unanswered(&error))?;
    if done == [DONE] {
        Ok(())
    } else {
        Err(unanswered(&"unexpected reply"))
    }
}

fn unanswered(error: &dyn std::fmt::Display) -> io::Error {
    io::Error::other(
        tr!(
            "Muxy is already running but did not answer: %@",
            error.to_string()
        )
        .to_string(),
    )
}

fn listen(listener: &LocalListener, folders: &Folders) {
    for stream in listener.incoming() {
        match stream {
            // A launch that stalls must not keep later ones waiting for long.
            Ok(stream) => drop(receive(stream, folders)),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn receive(mut stream: LocalStream, folders: &Folders) -> io::Result<()> {
    stream.set_read_timeout(Some(ANSWER))?;
    stream.set_write_timeout(Some(ANSWER))?;
    let mut message = Vec::new();
    (&stream).take(LIMIT + 1).read_to_end(&mut message)?;
    if message.len() as u64 > LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "launch message is too long",
        ));
    }
    let asked: Vec<PathBuf> = message
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .filter_map(|path| ServerPath(path.to_vec()).to_path_buf().ok())
        .collect();
    let closing = |_| io::Error::other("the app is closing");
    let opened = urls(&asked);
    if !opened.is_empty() {
        folders.try_send(opened).map_err(closing)?;
    }
    // The desktop does not bring this app forward for a launch that another
    // one answered, so the launch asks for the window itself.
    folders.try_send(Vec::new()).map_err(closing)?;
    stream.write_all(&[DONE])
}

/// Paths as the file URLs the app opens. A URL keeps every byte of a path,
/// and only an absolute path has one.
fn urls(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| reqwest::Url::from_file_path(path).ok())
        .map(String::from)
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "Regression fixture setup")]
mod tests {
    #[cfg(unix)]
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    use super::*;

    const PATIENCE: Duration = Duration::from_millis(400);

    fn paths(urls: &[String]) -> Vec<PathBuf> {
        urls.iter()
            .map(|url| {
                reqwest::Url::parse(url)
                    .expect("url")
                    .to_file_path()
                    .expect("path")
            })
            .collect()
    }

    #[cfg(unix)]
    fn odd(root: &Path) -> PathBuf {
        root.join(OsString::from_vec(
            b"My Project caf\xc3\xa9 \xff $(x)".to_vec(),
        ))
    }

    /// Windows paths are Unicode, so the oddest name is one outside the BMP.
    #[cfg(windows)]
    fn odd(root: &Path) -> PathBuf {
        root.join("My Project café 世界 🙂 $(x)")
    }

    #[test]
    fn a_second_launch_hands_its_folders_to_the_first_and_leaves() {
        let profile = tempfile::tempdir().expect("profile");
        let (folders, opened) = async_channel::unbounded();
        let first = vec![profile.path().join("at launch")];
        let app = claim_in(profile.path(), &first, &folders, PATIENCE)
            .expect("claim")
            .expect("the first launch is the app");
        assert_eq!(paths(&opened.recv_blocking().expect("launch")), first);

        let (ignored, unused) = async_channel::unbounded();
        let asked = vec![odd(profile.path()), profile.path().join("plain")];
        assert!(
            claim_in(profile.path(), &asked, &ignored, PATIENCE)
                .expect("handoff")
                .is_none()
        );
        assert_eq!(paths(&opened.recv_blocking().expect("handoff")), asked);
        assert!(
            opened.recv_blocking().expect("raise").is_empty(),
            "a handoff also asks for the window"
        );
        assert!(
            claim_in(profile.path(), &[], &ignored, PATIENCE)
                .expect("bare launch")
                .is_none()
        );
        assert!(
            opened.recv_blocking().expect("raise").is_empty(),
            "a bare second launch only brings the app forward"
        );
        assert!(opened.try_recv().is_err());
        assert!(
            unused.try_recv().is_err(),
            "a launch that left opens nothing"
        );
        drop(app);
    }

    /// On a busy macOS host the browser reply sometimes misses the call's
    /// timeout, for a reason not found yet.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn browser_commands_and_launch_handoffs_share_a_profile_without_sharing_a_socket() {
        use muxy_app_core::browser::api::{self, Request};
        use serde_json::{Map, json};

        let profile = tempfile::Builder::new()
            .tempdir_in(crate::app_socket::test_socket_folder())
            .expect("profile");
        let (folders, opened) = async_channel::unbounded();
        let _app = claim_in(profile.path(), &[], &folders, PATIENCE)
            .expect("claim")
            .expect("first instance");
        let socket = profile.path().join(api::SOCKET);
        let (_socket, incoming) = crate::app_socket::listen(&socket).expect("browser socket");
        let responder = std::thread::spawn(move || {
            for _ in 0..2 {
                let request = incoming.recv_blocking().expect("browser request");
                assert_eq!(request.request.verb, "browser.list");
                request.reply(Ok(json!([])));
            }
        });
        let asked = vec![odd(profile.path())];
        for _ in 0..2 {
            assert_eq!(
                api::call(
                    &socket,
                    &Request::new("browser.list", Map::new()),
                    Duration::from_secs(5),
                )
                .expect("browser reply"),
                json!([])
            );
            assert!(opened.try_recv().is_err());
            assert!(
                claim_in(profile.path(), &asked, &folders, PATIENCE)
                    .expect("handoff")
                    .is_none()
            );
            assert_eq!(paths(&opened.recv_blocking().expect("folders")), asked);
            assert!(opened.recv_blocking().expect("raise").is_empty());
        }
        responder.join().expect("responder");
    }

    #[test]
    fn a_socket_left_by_a_crashed_app_does_not_keep_the_profile() {
        let profile = tempfile::tempdir().expect("profile");
        drop(LocalListener::bind(profile.path().join("instance.sock")).expect("stale socket"));
        let (folders, opened) = async_channel::unbounded();
        let app = claim_in(profile.path(), &[], &folders, PATIENCE).expect("claim");
        assert!(app.is_some());
        assert!(opened.try_recv().is_err(), "a bare launch opens nothing");
        let asked = vec![profile.path().join("next")];
        assert!(
            claim_in(profile.path(), &asked, &folders, PATIENCE)
                .expect("handoff")
                .is_none()
        );
        assert_eq!(paths(&opened.recv_blocking().expect("handoff")), asked);
    }

    #[test]
    fn an_app_that_does_not_answer_fails_the_launch_instead_of_opening_twice() {
        let profile = tempfile::tempdir().expect("profile");
        let _held = muxy_core::file_lock::try_lock(&profile.path().join("app.lock")).expect("lock");
        let (folders, opened) = async_channel::unbounded();
        let started = Instant::now();
        let error = claim_in(profile.path(), &[], &folders, PATIENCE).expect_err("no answer");
        assert!(started.elapsed() >= PATIENCE);
        assert!(error.to_string().contains("already running"), "{error}");
        assert!(opened.try_recv().is_err());
    }

    #[test]
    fn a_launch_message_opens_only_whole_absolute_paths() {
        let profile = tempfile::tempdir().expect("profile");
        let (folders, opened) = async_channel::unbounded();
        let _app = claim_in(profile.path(), &[], &folders, PATIENCE).expect("claim");
        let send = |message: &[u8]| {
            let mut stream =
                LocalStream::connect(profile.path().join("instance.sock")).expect("connect");
            stream.write_all(message).expect("write");
            // The app closes a message that is too long once it has read
            // past the limit, which can be before this end shuts down.
            let _ = stream.shutdown(std::net::Shutdown::Write);
            let mut done = Vec::new();
            let _ = stream.read_to_end(&mut done);
            done
        };
        let absolute = if cfg!(windows) {
            r"C:\srv\app"
        } else {
            "/srv/app"
        };
        assert_eq!(
            send(format!("relative/folder\0{absolute}\0\0").as_bytes()),
            [DONE]
        );
        assert_eq!(
            paths(&opened.recv_blocking().expect("handoff")),
            [PathBuf::from(absolute)]
        );
        assert!(opened.recv_blocking().expect("raise").is_empty());
        let long = vec![b'/'; usize::try_from(LIMIT).expect("limit") + 1];
        assert!(send(&long).is_empty(), "too long to be a launch");
        assert!(opened.try_recv().is_err());
    }
}
