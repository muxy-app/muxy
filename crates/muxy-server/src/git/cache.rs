use super::Result;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How long a failed lookup is remembered, so an offline or GitHub-less
/// repository isn't asked again on every refresh.
const FAILED: Duration = Duration::from_secs(60);
const CAPACITY: usize = 128;

/// Remembers lookups, including failed ones, for a short time.
#[derive(Debug)]
pub(super) struct Cache<K, V> {
    ttl: Duration,
    entries: Mutex<HashMap<K, Entry<V>>>,
}

#[derive(Debug)]
struct Entry<V> {
    value: Result<V>,
    stored: Instant,
}

impl<V> Entry<V> {
    fn live(&self, ttl: Duration) -> bool {
        self.stored.elapsed() < if self.value.is_ok() { ttl } else { FAILED }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Cache<K, V> {
    pub(super) fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::default(),
        }
    }

    /// The remembered value for `key`, or what `lookup` finds now. `fresh`
    /// always looks up again, and remembers the answer.
    pub(super) fn get(&self, key: K, fresh: bool, lookup: impl FnOnce() -> Result<V>) -> Result<V> {
        if !fresh
            && let Some(entry) = self.entries().get(&key)
            && entry.live(self.ttl)
        {
            return entry.value.clone();
        }
        let value = lookup();
        let mut entries = self.entries();
        entries.retain(|_, entry| entry.live(self.ttl));
        if entries.len() >= CAPACITY
            && let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.stored)
                .map(|(key, _)| key.clone())
        {
            entries.remove(&oldest);
        }
        entries.insert(
            key,
            Entry {
                value: value.clone(),
                stored: Instant::now(),
            },
        );
        value
    }

    /// Drops what is remembered for the keys `matches` picks.
    pub(super) fn forget(&self, matches: impl Fn(&K) -> bool) {
        self.entries().retain(|key, _| !matches(key));
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<K, Entry<V>>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
