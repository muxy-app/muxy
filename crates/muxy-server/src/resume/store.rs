use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use muxy_protocol::SessionId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) argv: Vec<String>,
    pub(crate) cwd: Option<PathBuf>,
}

#[derive(Debug)]
enum Backend {
    Memory(Mutex<BTreeMap<SessionId, Record>>),
    Disk(PathBuf),
}

#[derive(Clone, Debug)]
pub(crate) struct Store(Arc<Backend>);

impl Default for Store {
    fn default() -> Self {
        Self(Arc::new(Backend::Memory(Mutex::default())))
    }
}

impl Store {
    pub(crate) fn open(directory: &Path) -> io::Result<Self> {
        let directory = directory.join("resume");
        muxy_core::files::private_dir(&directory)?;
        Ok(Self(Arc::new(Backend::Disk(directory))))
    }

    pub(crate) fn save(&self, session: SessionId, record: Option<&Record>) {
        let result = match (self.0.as_ref(), record) {
            (Backend::Memory(records), Some(record)) => {
                lock(records).insert(session, record.clone());
                Ok(())
            }
            (Backend::Memory(records), None) => {
                lock(records).remove(&session);
                Ok(())
            }
            (Backend::Disk(directory), Some(record)) => serde_json::to_vec(record)
                .map_err(io::Error::other)
                .and_then(|bytes| {
                    muxy_core::fs::write_atomic(&path(directory, session), &bytes, 0o600)
                        .map_err(io::Error::other)
                }),
            (Backend::Disk(directory), None) => remove(&path(directory, session)),
        };
        if let Err(error) = result {
            log::error!(
                "session {} could not save its resume record: {error}",
                session.get()
            );
        }
    }

    pub(crate) fn take(&self, session: SessionId) -> Option<Record> {
        match self.0.as_ref() {
            Backend::Memory(records) => lock(records).remove(&session),
            Backend::Disk(directory) => {
                let path = path(directory, session);
                let bytes = std::fs::read(&path).ok();
                if let Err(error) = remove(&path) {
                    log::error!(
                        "session {} could not remove its resume record: {error}",
                        session.get()
                    );
                }
                serde_json::from_slice(&bytes?).ok()
            }
        }
    }

    pub(crate) fn retain(&self, keep: impl Fn(SessionId) -> bool) {
        match self.0.as_ref() {
            Backend::Memory(records) => lock(records).retain(|session, _| keep(*session)),
            Backend::Disk(directory) => {
                let Ok(entries) = std::fs::read_dir(directory) else {
                    return;
                };
                for entry in entries.filter_map(Result::ok) {
                    let name = entry.file_name();
                    let session = name
                        .to_str()
                        .and_then(|name| name.strip_suffix(".json"))
                        .filter(|stem| stem.len() == 16)
                        .and_then(|stem| u64::from_str_radix(stem, 16).ok())
                        .and_then(SessionId::new);
                    if session.is_none_or(|session| !keep(session)) {
                        let _ = remove(&entry.path());
                    }
                }
            }
        }
    }
}

fn path(directory: &Path, session: SessionId) -> PathBuf {
    directory.join(format!("{:016x}.json", session.get()))
}

fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
