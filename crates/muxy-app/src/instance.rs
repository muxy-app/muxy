#[cfg(any(target_os = "linux", windows))]
use std::ffi::OsString;
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use muxy_app_core::session_link::SessionLink;
use muxy_protocol::ServerPath;
use muxy_protocol::transport::{LocalListener, LocalStream};
use muxy_ui::tr;

const LIMIT: u64 = 256 * 1024;
const ANSWER: Duration = Duration::from_secs(2);
const DONE: u8 = 1;

pub(crate) type Opened = async_channel::Sender<Vec<String>>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Asked {
    Folder(PathBuf),
    Link(String),
}

#[derive(Debug)]
pub(crate) struct Instance {
    _lock: muxy_core::file_lock::FileLock,
}

#[cfg(any(target_os = "linux", windows))]
pub(crate) fn claim(
    arguments: impl Iterator<Item = OsString>,
    opened: &Opened,
) -> io::Result<Option<Instance>> {
    let profile = muxy_core::dirs::muxy_dir()?;
    #[cfg(windows)]
    let arguments =
        arguments.filter(|argument| argument != muxy_ui::notifications::ACTIVATION_ARGUMENT);
    let asked: Vec<Asked> = arguments.filter_map(asked).collect();
    claim_in(&profile, &asked, opened, ANSWER)
}

#[cfg(any(target_os = "linux", windows))]
fn asked(argument: OsString) -> Option<Asked> {
    if let Some(text) = argument.to_str() {
        if SessionLink::parse(text).is_some() {
            return Some(Asked::Link(text.to_owned()));
        }
        if let Ok(url) = reqwest::Url::parse(text)
            && url.scheme() == "file"
        {
            return url.to_file_path().ok().map(Asked::Folder);
        }
    }
    std::path::absolute(argument).ok().map(Asked::Folder)
}

fn claim_in(
    profile: &Path,
    asked: &[Asked],
    opened: &Opened,
    patience: Duration,
) -> io::Result<Option<Instance>> {
    muxy_core::files::private_dir(profile)?;
    let socket = profile.join("instance.sock");
    match muxy_core::file_lock::try_lock(&profile.join("app.lock")) {
        Ok(lock) => {
            match std::fs::remove_file(&socket) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
            let listener = LocalListener::bind(&socket)?;
            #[cfg(unix)]
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
            if !asked.is_empty() {
                let _ = opened.try_send(urls(asked));
            }
            let opened = opened.clone();
            std::thread::Builder::new()
                .name("app-instance".into())
                .spawn(move || listen(&listener, &opened))?;
            Ok(Some(Instance { _lock: lock }))
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            hand_over(&socket, asked, patience)?;
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn hand_over(socket: &Path, asked: &[Asked], patience: Duration) -> io::Result<()> {
    let deadline = Instant::now() + patience;
    let mut stream = loop {
        match LocalStream::connect(socket) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => return Err(unanswered(&error)),
        }
    };
    let mut message = Vec::new();
    for entry in asked.iter().filter_map(|asked| match asked {
        Asked::Folder(folder) => ServerPath::from_path(folder).ok().map(|folder| folder.0),
        Asked::Link(link) => Some(link.clone().into_bytes()),
    }) {
        message.extend_from_slice(&entry);
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

fn listen(listener: &LocalListener, opened: &Opened) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => drop(receive(stream, opened)),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn receive(mut stream: LocalStream, opened: &Opened) -> io::Result<()> {
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
    let asked: Vec<Asked> = message
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| match std::str::from_utf8(entry) {
            Ok(text) if SessionLink::parse(text).is_some() => Some(Asked::Link(text.to_owned())),
            _ => ServerPath(entry.to_vec())
                .to_path_buf()
                .ok()
                .map(Asked::Folder),
        })
        .collect();
    let closing = |_| io::Error::other("the app is closing");
    opened.try_send(urls(&asked)).map_err(closing)?;
    stream.write_all(&[DONE])
}

fn urls(asked: &[Asked]) -> Vec<String> {
    asked
        .iter()
        .filter_map(|asked| match asked {
            Asked::Folder(path) => reqwest::Url::from_file_path(path).ok().map(String::from),
            Asked::Link(link) => Some(link.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(session: u64) -> String {
        SessionLink {
            session: muxy_protocol::SessionId::new(session).expect("session"),
            server: None,
            split: None,
        }
        .to_link()
    }

    #[test]
    fn launch_arguments_open_links_file_urls_and_folders() {
        let folder = crate::test_support::directory();
        let url = reqwest::Url::from_directory_path(&folder).expect("file URL");
        assert_eq!(asked(link(7).into()), Some(Asked::Link(link(7))));
        assert_eq!(
            asked(url.to_string().into()),
            Some(Asked::Folder(folder.clone()))
        );
        assert_eq!(
            asked(folder.clone().into_os_string()),
            Some(Asked::Folder(folder))
        );
        assert!(matches!(
            asked("muxy-other://session/7".into()),
            Some(Asked::Folder(_))
        ));
    }

    #[test]
    fn a_second_launch_hands_its_links_and_folders_to_the_running_app() {
        let profile = crate::test_support::directory();
        let folder = crate::test_support::directory();
        let (opened, urls) = async_channel::unbounded();
        let _running = claim_in(&profile, &[], &opened, ANSWER)
            .expect("first launch")
            .expect("first launch keeps running");
        let (unused, _) = async_channel::unbounded();
        let asked = [Asked::Link(link(7)), Asked::Folder(folder.clone())];
        assert!(
            claim_in(&profile, &asked, &unused, ANSWER)
                .expect("hand over")
                .is_none()
        );
        assert_eq!(
            urls.recv_blocking().expect("opened"),
            [
                link(7),
                reqwest::Url::from_file_path(&folder)
                    .expect("file URL")
                    .to_string()
            ]
        );
        assert!(urls.try_recv().is_err());
    }
}
