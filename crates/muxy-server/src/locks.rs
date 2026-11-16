use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, PoisonError, Weak};

#[derive(Debug, Default)]
pub(crate) struct KeyedLocks<K, L> {
    entries: Mutex<HashMap<K, Weak<L>>>,
}

impl<K: Eq + Hash, L: Default> KeyedLocks<K, L> {
    pub(crate) fn get(&self, key: K) -> Arc<L> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.retain(|_, lock| lock.strong_count() > 0);
        let lock = entries
            .get(&key)
            .and_then(Weak::upgrade)
            .unwrap_or_default();
        entries.insert(key, Arc::downgrade(&lock));
        lock
    }
}
