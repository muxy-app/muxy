mod expiry;
mod git;
mod mutations;
pub(crate) use git::GitReceipt;
mod recovery;
pub(crate) mod retention;
mod sessions;
mod storage;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::notifier::Notifier;
use crate::{ServerError, archive::Archive};
use muxy_protocol::{
    CATALOG_PAGE_SIZE, CatalogPage, ErrorCode, ErrorReply, MAX_PROJECTS, OperationId,
    ProjectDescriptor, ProjectId, ProjectIntent, ServerIdentity, ServerPath, SessionId,
    SessionInfo, SessionStatus,
};

#[derive(Clone, Debug, Default)]
pub struct LegacyImport {
    pub projects: Vec<ProjectDescriptor>,
    pub sessions: BTreeMap<SessionId, ProjectId>,
}

#[derive(Clone, Debug)]
struct Creation {
    info: SessionInfo,
    error: Option<ErrorReply>,
}

#[derive(Clone, Debug)]
struct Membership {
    info: SessionInfo,
    status: SessionStatus,
    ended_at: Option<u64>,
}

#[derive(Clone, Debug)]
struct Receipt {
    intent: ProjectIntent,
    error: Option<ErrorReply>,
    complete: bool,
}

#[derive(Clone, Debug)]
struct State {
    git: BTreeMap<OperationId, GitReceipt>,
    version: u32,
    server: ServerIdentity,
    home: ProjectId,
    legacy_home: Option<ProjectId>,
    revision: u64,
    projects: BTreeMap<ProjectId, ProjectDescriptor>,
    deleting: BTreeSet<ProjectId>,
    discarding: BTreeSet<SessionId>,
    cancelled: BTreeSet<OperationId>,
    sessions: BTreeMap<SessionId, Membership>,
    creations: BTreeMap<OperationId, Creation>,
    receipts: BTreeMap<OperationId, Receipt>,
    closes: BTreeMap<OperationId, (SessionId, bool)>,
    finished: expiry::Finished,
}

#[derive(Debug)]
pub(crate) struct Catalog {
    state: Mutex<State>,
    write: Mutex<()>,
    path: Option<PathBuf>,
    revision: AtomicU64,
    pending_exits: Mutex<recovery::PendingExits>,
    notifier: Arc<Notifier>,
}

impl State {
    fn fresh(home: &Path) -> Self {
        let project = ProjectDescriptor::home(ServerPath::from(home));
        let id = project.id;
        Self {
            git: BTreeMap::new(),
            version: 1,
            server: ServerIdentity::new(),
            home: id,
            legacy_home: None,
            revision: 0,
            projects: BTreeMap::from([(id, project)]),
            deleting: BTreeSet::new(),
            discarding: BTreeSet::new(),
            cancelled: BTreeSet::new(),
            sessions: BTreeMap::new(),
            creations: BTreeMap::new(),
            receipts: BTreeMap::new(),
            closes: BTreeMap::new(),
            finished: expiry::Finished::default(),
        }
    }

    fn validate(&self) -> Result<(), ServerError> {
        if self.version != 1
            || self.projects.len() > MAX_PROJECTS
            || self
                .projects
                .values()
                .filter(|project| project.home)
                .count()
                != 1
            || !self.projects.get(&self.home).is_some_and(|home| home.home)
            || self.deleting.contains(&self.home)
        {
            return Err(bad("invalid catalog identity, Home, size or version"));
        }
        for (id, project) in &self.projects {
            project
                .validate()
                .map_err(|_| bad("invalid project descriptor"))?;
            if *id != project.id {
                return Err(bad("project key disagrees with identity"));
            }
            self.validate_parent(project)?;
        }
        for (id, membership) in &self.sessions {
            if *id != membership.info.id || !self.projects.contains_key(&membership.info.project) {
                return Err(bad("session has no owning project"));
            }
        }
        Ok(())
    }

    fn validate_parent(&self, project: &ProjectDescriptor) -> Result<(), ServerError> {
        if let Some(parent) = project.parent_id {
            let parent = self
                .projects
                .get(&parent)
                .ok_or_else(|| bad("missing project parent"))?;
            if parent.home
                || parent.parent_id.is_some()
                || parent.kind == Some(muxy_protocol::ProjectKind::Worktree)
            {
                return Err(bad("invalid project parent"));
            }
        }
        Ok(())
    }

    fn import(&mut self, legacy: LegacyImport, archive: &Archive) -> io::Result<()> {
        let homes: Vec<_> = legacy
            .projects
            .iter()
            .filter(|project| project.home)
            .collect();
        if homes.len() > 1 {
            return Err(io::Error::other(
                "legacy snapshot has multiple Home projects",
            ));
        }
        if let Some(home) = homes.first() {
            self.legacy_home = Some(home.id);
            self.projects.remove(&self.home);
            self.home = home.id;
        }
        for project in legacy.projects {
            if self.projects.insert(project.id, project).is_some() {
                return Err(io::Error::other("duplicate legacy project identity"));
            }
        }
        for id in archive
            .ids()?
            .into_iter()
            .chain(legacy.sessions.keys().copied())
            .collect::<BTreeSet<_>>()
        {
            let project = legacy.sessions.get(&id).copied().unwrap_or(self.home);
            let directory = self
                .projects
                .get(&project)
                .ok_or_else(|| io::Error::other("unknown legacy session owner"))?
                .directory
                .clone();
            self.sessions.insert(
                id,
                Membership {
                    info: SessionInfo {
                        id,
                        project,
                        directory,
                    },
                    ended_at: None,
                    status: if archive.contains(id) {
                        SessionStatus::Ended
                    } else {
                        SessionStatus::Unavailable
                    },
                },
            );
        }
        self.validate().map_err(io::Error::other)
    }
}

impl Catalog {
    pub(crate) fn identity(&self) -> ServerIdentity {
        self.lock().server
    }

    pub(crate) fn memory(notifier: Arc<Notifier>) -> Self {
        Self::from_state(
            State::fresh(&std::env::home_dir().unwrap_or_else(|| PathBuf::from("/"))),
            None,
            notifier,
        )
    }

    fn from_state(state: State, path: Option<PathBuf>, notifier: Arc<Notifier>) -> Self {
        Self {
            revision: AtomicU64::new(state.revision),
            state: Mutex::new(state),
            write: Mutex::new(()),
            path,
            pending_exits: Mutex::default(),
            notifier,
        }
    }

    pub(crate) fn open(
        directory: &Path,
        archive: &Archive,
        legacy: LegacyImport,
        notifier: Arc<Notifier>,
    ) -> io::Result<Self> {
        let path = directory.join("catalog.json");
        let mut state = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<storage::StoredState>(&bytes)?.into(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let home = std::env::home_dir()
                    .ok_or_else(|| io::Error::other("home directory unavailable"))?;
                let mut state = State::fresh(&home);
                state.import(legacy, archive)?;
                state
            }
            Err(error) => return Err(error),
        };
        state.validate().map_err(io::Error::other)?;
        let now = retention::now();
        for membership in state.sessions.values_mut() {
            membership.ended_at.get_or_insert(now);
            membership.status = if archive.contains(membership.info.id) {
                SessionStatus::Ended
            } else {
                SessionStatus::Unavailable
            };
        }
        let catalog = Self::from_state(state, Some(path), notifier);
        catalog.update(|_| Ok(())).map_err(io::Error::other)?;
        Ok(catalog)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn update<T>(
        &self,
        edit: impl FnOnce(&mut State) -> Result<T, ServerError>,
    ) -> Result<T, ServerError> {
        let write = self.write.lock().unwrap_or_else(PoisonError::into_inner);
        let mut next = self.lock().clone();
        let result = edit(&mut next)?;
        next.expire_records(expiry::now_millis());
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or_else(|| bad("catalog revisions exhausted"))?;
        next.validate()?;
        let revision = next.revision;
        let saved = self.path.as_ref().map_or(Ok(()), |path| save(path, &next));
        let published = saved.as_ref().map_or_else(|error| error.renamed, |()| true);
        if published {
            self.revision.store(revision, Ordering::Release);
            *self.lock() = next;
        }
        drop(write);
        if published {
            self.notifier.catalog_changed(revision);
        }
        saved.map_err(|error| storage_error(&error.error))?;
        Ok(result)
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    pub(crate) fn ensure_durable(&self) -> Result<(), ServerError> {
        let _write = self.write.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(path) = &self.path {
            muxy_core::fs::sync_parent(path).map_err(|error| storage_error(&error))?;
        }
        Ok(())
    }
    pub(crate) fn home(&self) -> ProjectId {
        self.lock().home
    }

    pub(crate) fn page(
        &self,
        after: Option<ProjectId>,
        revision: Option<u64>,
    ) -> Result<CatalogPage, ServerError> {
        self.retry_exits();
        let state = self.lock();
        check_revision(&state, revision)?;
        let mut entries = state.projects.values().filter(|project| {
            !state.deleting.contains(&project.id) && after.is_none_or(|after| project.id > after)
        });
        let mut bytes = 0;
        let mut projects = Vec::new();
        let mut entries = entries.by_ref().peekable();
        while projects.len() < CATALOG_PAGE_SIZE {
            let Some(project) = entries.peek() else {
                break;
            };
            let size = project.logo.as_ref().map_or(0, |logo| logo.len()) + 8192;
            if bytes + size > 4 * 1024 * 1024 {
                break;
            }
            bytes += size;
            projects.push((*project).clone());
            entries.next();
        }
        let next = entries
            .next()
            .and_then(|_| projects.last().map(|project| project.id));
        Ok(CatalogPage {
            server: state.server,
            home: state.home,
            revision: state.revision,
            projects,
            next,
            legacy_home: state.legacy_home,
        })
    }
}

fn bad(message: impl Into<String>) -> ServerError {
    ServerError::new(ErrorCode::BadRequest, message)
}
fn unknown() -> ServerError {
    ServerError::new(ErrorCode::UnknownProject, "project no longer exists")
}
fn check_revision(state: &State, revision: Option<u64>) -> Result<(), ServerError> {
    if revision.is_some_and(|revision| revision != state.revision) {
        Err(ServerError::new(
            ErrorCode::CatalogChanged,
            "catalog changed; restart the page fetch",
        ))
    } else {
        Ok(())
    }
}

fn save(path: &Path, state: &State) -> Result<(), muxy_core::fs::WriteError> {
    let bytes =
        serde_json::to_vec(&storage::StoredState::try_from(state)?).map_err(io::Error::from)?;
    muxy_core::fs::write_atomic(path, &bytes, 0o600)
}

fn storage_error(error: &io::Error) -> ServerError {
    ServerError::new(
        ErrorCode::PersistenceFailed,
        format!("could not save project catalog: {error}"),
    )
}
