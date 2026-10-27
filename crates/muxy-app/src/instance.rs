//! One desktop app per profile. A second launch hands the folders it was
//! asked to open to the first and exits, which is also how `muxy <folder>`
//! reaches an app that is already running.

use std::ffi::{OsStr, OsString};
use std::fs::DirBuilder;
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
#[cfg(target_os = "linux")]
pub(crate) fn claim(
    arguments: impl Iterator<Item = OsString>,
    folders: &Folders,
) -> io::Result<Option<Instance>> {
    let state = muxy_app_core::store::default_path().map_err(io::Error::other)?;
    let profile = state
        .parent()
        .ok_or_else(|| io::Error::other("Missing profile directory"))?;
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
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(profile)?;
    let socket = profile.join("instance.sock");
    match muxy_core::file_lock::try_lock(&profile.join("app.lock")) {
        Ok(lock) => {
            // Only a crashed app leaves its socket behind while the lock is free.
            match std::fs::remove_file(&socket) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
            let listener = UnixListener::bind(&socket)?;
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
        match UnixStream::connect(socket) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => return Err(unanswered(&error)),
        }
    };
    let mut message = Vec::new();
    for folder in asked {
        message.extend_from_slice(folder.as_os_str().as_bytes());
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

fn listen(listener: &UnixListener, folders: &Folders) {
    for stream in listener.incoming() {
        match stream {
            // A launch that stalls must not keep later ones waiting for long.
            Ok(stream) => drop(receive(stream, folders)),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn receive(mut stream: UnixStream, folders: &Folders) -> io::Result<()> {
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
        .map(|path| PathBuf::from(OsStr::from_bytes(path)))
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
