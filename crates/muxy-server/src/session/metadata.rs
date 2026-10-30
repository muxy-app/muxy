#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
use linux::{agent_provider, foreground_member, process_directory};
#[cfg(target_os = "macos")]
use macos::{agent_provider, foreground_member, process_directory};

use muxy_protocol::{ForegroundProcess, MetadataEvent, ServerPath};
use muxy_terminal::TerminalEvent;
use muxy_terminal::pty::Pty;

pub(super) struct Metadata {
    pub(super) title: String,
    pub(super) directory: ServerPath,
    pub(super) process: Option<ForegroundProcess>,
    pub(super) agent: Option<muxy_protocol::AgentProvider>,
    /// The session's own shell is in the foreground, so no program it ran is.
    pub(super) shell_foreground: bool,
    next_agent: std::time::Instant,
    agent_group: Option<u32>,
    observed_directory: Option<ServerPath>,
}

impl Metadata {
    pub(super) fn new(directory: ServerPath) -> Self {
        Self {
            title: String::new(),
            directory,
            process: None,
            agent: None,
            shell_foreground: false,
            next_agent: std::time::Instant::now(),
            agent_group: None,
            observed_directory: None,
        }
    }

    pub(super) fn update(&mut self, pty: &Pty, terminal: Vec<TerminalEvent>) -> Vec<MetadataEvent> {
        let mut events = Vec::new();
        let previous_directory = self.directory.clone();
        let group = pty.foreground_pid();
        let member = group.and_then(foreground_member);
        self.shell_foreground = group == Some(pty.child_pid())
            && member.as_ref().is_some_and(|(_, name)| {
                matches!(
                    name.trim_start_matches('-'),
                    "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh"
                )
            });
        if self.next_agent <= std::time::Instant::now() || self.agent_group != group {
            self.next_agent = std::time::Instant::now() + std::time::Duration::from_millis(500);
            self.agent_group = group;
            self.agent = if self.shell_foreground {
                None
            } else {
                group.and_then(agent_provider)
            };
        }
        if let Some(group) = group {
            let process = ForegroundProcess {
                name: member
                    .as_ref()
                    .map_or_else(String::new, |(_, name)| name.clone()),
                is_shell: group == pty.child_pid(),
            };
            if self.process.as_ref() != Some(&process) {
                events.push(MetadataEvent::ForegroundProcess {
                    name: process.name.clone(),
                    is_shell: process.is_shell,
                });
                self.process = Some(process);
            }
            if let Some(directory) = member.and_then(|(pid, _)| process_directory(pid))
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
                    if let Some(directory) = terminal_directory(&directory) {
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

fn terminal_directory(value: &[u8]) -> Option<ServerPath> {
    if value.starts_with(b"/") {
        return (!value.contains(&0)).then(|| ServerPath(value.to_vec()));
    }
    let rest = value.strip_prefix(b"file://")?;
    let path = &rest[rest.iter().position(|byte| *byte == b'/')? + 1..];
    let mut bytes = path.iter().copied();
    let mut path = vec![b'/'];
    while let Some(byte) = bytes.next() {
        let byte = if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            u8::try_from(high * 16 + low).ok()?
        } else {
            byte
        };
        if byte == 0 {
            return None;
        }
        path.push(byte);
    }
    Some(ServerPath(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_directories_keep_raw_and_percent_encoded_bytes() {
        assert_eq!(
            terminal_directory(b"file://host/tmp/caf\xe9"),
            Some(ServerPath(b"/tmp/caf\xe9".to_vec()))
        );
        assert_eq!(
            terminal_directory(b"file://host/tmp/caf%E9"),
            Some(ServerPath(b"/tmp/caf\xe9".to_vec()))
        );
        assert_eq!(
            terminal_directory(b"/plain/path"),
            Some(ServerPath(b"/plain/path".to_vec()))
        );
        assert_eq!(terminal_directory(b"file://host/a%00b"), None);
        assert_eq!(terminal_directory(b"relative"), None);
    }
}
