use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::run;

const TTL: Duration = Duration::from_secs(300);
const CAPACITY: usize = 128;

#[derive(Debug)]
struct Entry {
    remotes: Vec<u8>,
    branch: String,
    stored: Instant,
}

#[derive(Debug, Default)]
pub(super) struct DefaultBranches {
    entries: Mutex<HashMap<PathBuf, Entry>>,
}

impl DefaultBranches {
    pub(super) fn resolve(
        &self,
        repository: &Path,
        resolve: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        let remotes = run(
            repository,
            &["config", "--get-regexp", r"^remote\..*\.url$"],
        )
        .unwrap_or_default();
        {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            entries.retain(|_, entry| entry.stored.elapsed() < TTL);
            if let Some(entry) = entries.get(repository)
                && entry.remotes == remotes
            {
                return Some(entry.branch.clone());
            }
        }
        let branch = resolve()?;
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if entries.len() >= CAPACITY
            && let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.stored)
                .map(|(path, _)| path.clone())
        {
            entries.remove(&oldest);
        }
        entries.insert(
            repository.to_owned(),
            Entry {
                remotes,
                branch: branch.clone(),
                stored: Instant::now(),
            },
        );
        Some(branch)
    }
}
