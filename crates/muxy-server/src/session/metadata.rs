#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
use linux::{agent_provider, foreground_member, process_directory};
#[cfg(target_os = "macos")]
use macos::{agent_provider, foreground_member, process_directory};
#[cfg(windows)]
use windows::Foreground;

use muxy_protocol::{ForegroundProcess, MetadataEvent, ServerPath};
use muxy_terminal::TerminalEvent;

use crate::spawn::Shell;

pub(super) struct Metadata {
    pub(super) title: String,
    pub(super) directory: ServerPath,
    pub(super) process: Option<ForegroundProcess>,
    pub(super) agent: Option<muxy_protocol::AgentProvider>,
    next_agent: std::time::Instant,
    agent_group: Option<u32>,
    observed_directory: Option<ServerPath>,
}

/// The program in a session's foreground: the PTY's foreground process
/// group on Unix, the newest program the shell started on Windows.
#[cfg(unix)]
struct Foreground {
    group: u32,
    member: Option<(i32, String)>,
    is_shell: bool,
}

#[cfg(unix)]
impl Foreground {
    fn of(shell: &Shell) -> Option<Self> {
        let group = shell.pty.foreground_pid()?;
        Some(Self {
            group,
            member: foreground_member(group),
            is_shell: group == shell.pty.child_pid(),
        })
    }

    fn name(&self) -> String {
        self.member
            .as_ref()
            .map_or_else(String::new, |(_, name)| name.clone())
    }

    /// The shell itself waiting at its prompt, which is never an agent.
    fn is_idle_shell(&self) -> bool {
        self.is_shell
            && self.member.as_ref().is_some_and(|(_, name)| {
                matches!(
                    name.trim_start_matches('-'),
                    "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh"
                )
            })
    }

    fn agent(&self) -> Option<muxy_protocol::AgentProvider> {
        agent_provider(self.group)
    }

    fn directory(&self) -> Option<ServerPath> {
        self.member
            .as_ref()
            .and_then(|(pid, _)| process_directory(*pid))
    }
}

impl Metadata {
    pub(super) fn new(directory: ServerPath) -> Self {
        Self {
            title: String::new(),
            directory,
            process: None,
            agent: None,
            next_agent: std::time::Instant::now(),
            agent_group: None,
            observed_directory: None,
        }
    }

    pub(super) fn update(
        &mut self,
        shell: &Shell,
        terminal: Vec<TerminalEvent>,
    ) -> Vec<MetadataEvent> {
        let foreground = Foreground::of(shell);
        let mut events = Vec::new();
        let previous_directory = self.directory.clone();
        let group = foreground.as_ref().map(|foreground| foreground.group);
        if self.next_agent <= std::time::Instant::now() || self.agent_group != group {
            self.next_agent = std::time::Instant::now() + std::time::Duration::from_millis(500);
            self.agent_group = group;
            self.agent = foreground
                .as_ref()
                .filter(|foreground| !foreground.is_idle_shell())
                .and_then(Foreground::agent);
        }
        if let Some(foreground) = foreground {
            let process = ForegroundProcess {
                name: foreground.name(),
                is_shell: foreground.is_shell,
            };
            if self.process.as_ref() != Some(&process) {
                events.push(MetadataEvent::ForegroundProcess {
                    name: process.name.clone(),
                    is_shell: process.is_shell,
                });
                self.process = Some(process);
            }
            if let Some(directory) = foreground.directory()
                && self.observed_directory.as_ref() != Some(&directory)
            {
                self.directory.clone_from(&directory);
                self.observed_directory = Some(directory);
            }
        }
        for event in terminal {
            match event {
                TerminalEvent::Title(title) if title != self.title => {
                    self.title.clone_from(&title);
                    events.push(MetadataEvent::Title(title));
                }
                TerminalEvent::Directory(directory) => {
                    if let Some(directory) = terminal_directory(&directory, cfg!(windows)) {
                        self.directory = directory;
                    }
                }
                TerminalEvent::ClipboardWrite(text) => {
                    events.push(MetadataEvent::ClipboardWrite(text));
                }
                TerminalEvent::Bell => events.push(MetadataEvent::Bell),
                TerminalEvent::Progress(_) | TerminalEvent::Title(_) => {}
            }
        }
        if self.directory != previous_directory {
            events.push(MetadataEvent::Directory(self.directory.clone()));
        }
        events
    }
}

/// The folder an OSC 7 report names. Windows takes only drive paths, as
/// `file:///C:/…`, `file://host/C:/…` or `C:\…`.
fn terminal_directory(value: &str, windows: bool) -> Option<ServerPath> {
    let path = if let Some(url) = value.strip_prefix("file://") {
        let (_, path) = url.split_once('/')?;
        let mut path = percent_decoded(path)?;
        if !windows {
            path.insert(0, b'/');
        }
        path
    } else {
        value.as_bytes().to_vec()
    };
    if path.contains(&0) {
        return None;
    }
    if !windows {
        return path.starts_with(b"/").then_some(ServerPath(path));
    }
    let path = String::from_utf8(path).ok()?.replace('/', "\\");
    let drive = path.as_bytes();
    (drive.len() >= 2
        && drive[0].is_ascii_alphabetic()
        && drive[1] == b':'
        && drive.get(2).is_none_or(|separator| *separator == b'\\'))
    .then(|| {
        if path.len() == 2 {
            ServerPath(format!("{path}\\").into_bytes())
        } else {
            ServerPath(path.into_bytes())
        }
    })
}

fn percent_decoded(text: &str) -> Option<Vec<u8>> {
    let mut bytes = text.bytes();
    let mut decoded = Vec::with_capacity(text.len());
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            u8::try_from(high * 16 + low).ok()?
        } else {
            byte
        });
    }
    Some(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_reports_keep_their_bytes() {
        let unix = |value| terminal_directory(value, false).map(|path| path.0);
        assert_eq!(unix("file:///tmp/a%20b"), Some(b"/tmp/a b".to_vec()));
        assert_eq!(unix("file://host/tmp/%FF"), Some(b"/tmp/\xff".to_vec()));
        assert_eq!(unix("/tmp"), Some(b"/tmp".to_vec()));
        assert_eq!(unix("file:///tmp/%00"), None);
        assert_eq!(unix("relative"), None);
    }

    #[test]
    fn windows_reports_name_drive_folders_only() {
        let windows = |value| terminal_directory(value, true).map(|path| path.0);
        assert_eq!(
            windows("file:///C:/Users/a%20b"),
            Some(br"C:\Users\a b".to_vec())
        );
        assert_eq!(windows("file://desk/d:/src"), Some(br"d:\src".to_vec()));
        assert_eq!(windows("file:///C:"), Some(br"C:\".to_vec()));
        assert_eq!(windows(r"C:\Windows"), Some(br"C:\Windows".to_vec()));
        assert_eq!(
            windows("file:///C:/%C3%A9"),
            Some("C:\\é".as_bytes().to_vec())
        );
        for refused in [
            "file:///home/me",
            "file://server/share/x",
            "file:///C:/%FF",
            "file:///C:/a%00",
            "/tmp",
            "C:relative",
        ] {
            assert_eq!(windows(refused), None, "{refused}");
        }
    }
}
