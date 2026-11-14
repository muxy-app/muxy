//! What each session is called, worked out the way the desktop app does: the
//! name a renamed tab gave it, else the title its program set, else the
//! running program's name, else its folder.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use muxy_protocol::{ForegroundProcess, MetadataEvent, ServerPath, SessionId, SessionMetadata};

#[derive(Default)]
pub(crate) struct Titles {
    state: Mutex<HashMap<SessionId, Known>>,
    /// The server sends every session's metadata, so an attached terminal's
    /// own updates add nothing.
    watching: AtomicBool,
}

struct Known {
    name: Option<String>,
    /// The folder the session list gave, until metadata arrives.
    directory: ServerPath,
    metadata: Option<SessionMetadata>,
}

impl Default for Known {
    fn default() -> Self {
        Self {
            name: None,
            directory: ServerPath(Vec::new()),
            metadata: None,
        }
    }
}

impl Known {
    fn title(&self) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }
        match &self.metadata {
            Some(metadata) => muxy_protocol::title::derive(
                &metadata.title,
                metadata.process.as_ref(),
                &metadata.directory,
            ),
            None => muxy_protocol::title::derive("", None, &self.directory),
        }
    }
}

impl Titles {
    pub(crate) fn watching(&self) {
        self.watching.store(true, Ordering::Release);
    }

    /// Records the name a session list gave `session` and returns its title.
    pub(crate) fn listed(
        &self,
        session: SessionId,
        name: Option<String>,
        directory: &ServerPath,
    ) -> String {
        let mut state = self.lock();
        let known = state.entry(session).or_default();
        known.name = name;
        known.directory.clone_from(directory);
        known.title()
    }

    /// Applies metadata the server sent for any session; returns the new
    /// title when it changed.
    pub(crate) fn metadata(&self, session: SessionId, metadata: SessionMetadata) -> Option<String> {
        self.update(session, |known| known.metadata = Some(metadata))
    }

    /// Applies what an attach reported, unless the server sends every
    /// session's metadata anyway.
    pub(crate) fn attached(
        &self,
        session: SessionId,
        title: &str,
        directory: &ServerPath,
        process: Option<&ForegroundProcess>,
    ) -> Option<String> {
        if self.watching.load(Ordering::Acquire) {
            return None;
        }
        self.metadata(
            session,
            SessionMetadata {
                title: title.to_owned(),
                directory: directory.clone(),
                process: process.cloned(),
            },
        )
    }

    /// Applies an attached terminal's metadata event, unless the server sends
    /// every session's metadata anyway.
    pub(crate) fn event(&self, session: SessionId, event: &MetadataEvent) -> Option<String> {
        if self.watching.load(Ordering::Acquire) {
            return None;
        }
        self.update(session, |known| {
            let directory = known.directory.clone();
            let metadata = known.metadata.get_or_insert_with(|| SessionMetadata {
                title: String::new(),
                directory,
                process: None,
            });
            match event {
                MetadataEvent::Title(title) => metadata.title.clone_from(title),
                MetadataEvent::Directory(directory) => metadata.directory = directory.clone(),
                MetadataEvent::ForegroundProcess { name, is_shell } => {
                    metadata.process = Some(ForegroundProcess {
                        name: name.clone(),
                        is_shell: *is_shell,
                    });
                }
                _ => {}
            }
        })
    }

    pub(crate) fn ended(&self, session: SessionId) {
        self.lock().remove(&session);
    }

    fn update(&self, session: SessionId, edit: impl FnOnce(&mut Known)) -> Option<String> {
        let mut state = self.lock();
        let known = state.entry(session).or_default();
        let before = known.title();
        edit(known);
        let after = known.title();
        (before != after).then_some(after)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<SessionId, Known>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SessionId {
        SessionId::new(7).expect("nonzero")
    }

    fn path(text: &str) -> ServerPath {
        ServerPath(text.as_bytes().to_vec())
    }

    fn metadata(title: &str, process: Option<(&str, bool)>) -> SessionMetadata {
        SessionMetadata {
            title: title.into(),
            directory: path("/home/dev/api"),
            process: process.map(|(name, is_shell)| ForegroundProcess {
                name: name.into(),
                is_shell,
            }),
        }
    }

    #[test]
    fn titles_follow_the_desktop_order() {
        let titles = Titles::default();
        assert_eq!(
            titles.listed(session(), None, &path("/home/dev/api")),
            "api"
        );
        assert_eq!(
            titles.metadata(session(), metadata("", Some(("zsh", true)))),
            None,
            "a shell at its prompt keeps the folder"
        );
        assert_eq!(
            titles.metadata(session(), metadata("", Some(("lazygit", false)))),
            Some("lazygit".into())
        );
        assert_eq!(
            titles.metadata(session(), metadata("vim notes.md", Some(("vim", false)))),
            Some("vim notes.md".into())
        );
        assert_eq!(
            titles.listed(session(), Some("API server".into()), &path("/home/dev/api")),
            "API server"
        );
        assert_eq!(
            titles.metadata(session(), metadata("", Some(("lazygit", false)))),
            None,
            "a name outranks whatever runs"
        );
        assert_eq!(
            titles.listed(session(), None, &path("/home/dev/api")),
            "lazygit"
        );
    }

    #[test]
    fn an_attached_terminal_titles_its_session_only_without_a_watch() {
        let titles = Titles::default();
        let lazygit = ForegroundProcess {
            name: "lazygit".into(),
            is_shell: false,
        };
        assert_eq!(
            titles.attached(session(), "", &path("/home/dev/api"), Some(&lazygit)),
            Some("lazygit".into())
        );
        assert_eq!(
            titles.event(session(), &MetadataEvent::Title("htop".into())),
            Some("htop".into())
        );

        let watched = Titles::default();
        watched.watching();
        assert_eq!(
            watched.attached(session(), "", &path("/home/dev/api"), Some(&lazygit)),
            None
        );
        assert_eq!(
            watched.event(session(), &MetadataEvent::Title("htop".into())),
            None
        );
    }
}
