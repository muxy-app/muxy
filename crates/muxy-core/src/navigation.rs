const MAX_ENTRIES: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NavigationEntry {
    pub project_id: String,
    pub worktree_id: String,
    pub area_id: String,
    pub tab_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Back,
    Forward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationTarget {
    pub index: usize,
    pub entry: NavigationEntry,
}

#[derive(Debug, Clone, Default)]
pub struct NavigationHistory {
    entries: Vec<NavigationEntry>,
    cursor: Option<usize>,
}

impl NavigationHistory {
    pub fn record(&mut self, entry: NavigationEntry) {
        if self.current() == Some(&entry) {
            return;
        }
        let keep = self.cursor.map_or(0, |cursor| cursor + 1);
        self.entries.truncate(keep);
        self.entries.push(entry);
        self.cursor = Some(self.entries.len() - 1);
        if self.entries.len() > MAX_ENTRIES {
            let overflow = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..overflow);
            self.cursor = self.cursor.map(|cursor| cursor - overflow);
        }
    }

    pub fn target(
        &self,
        direction: Direction,
        mut is_live: impl FnMut(&NavigationEntry) -> bool,
    ) -> Option<NavigationTarget> {
        let cursor = self.cursor?;
        let mut indices: Box<dyn Iterator<Item = usize>> = match direction {
            Direction::Back => Box::new((0..cursor).rev()),
            Direction::Forward => Box::new(cursor + 1..self.entries.len()),
        };
        indices.find_map(|index| {
            let entry = &self.entries[index];
            is_live(entry).then(|| NavigationTarget {
                index,
                entry: entry.clone(),
            })
        })
    }

    pub fn commit_target(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.cursor = Some(index);
        true
    }

    pub fn prune(&mut self, mut keep: impl FnMut(&NavigationEntry) -> bool) {
        let previous_cursor = self.cursor;
        let mut entries = Vec::with_capacity(self.entries.len());
        let mut cursor = None;
        for (index, entry) in self.entries.drain(..).enumerate() {
            if !keep(&entry) {
                continue;
            }
            entries.push(entry);
            if previous_cursor.is_some_and(|previous| index <= previous) {
                cursor = Some(entries.len() - 1);
            }
        }
        self.entries = entries;
        self.cursor = if self.entries.is_empty() {
            None
        } else {
            cursor.or(Some(0))
        };
    }

    pub fn can_navigate(
        &self,
        direction: Direction,
        is_live: impl FnMut(&NavigationEntry) -> bool,
    ) -> bool {
        self.target(direction, is_live).is_some()
    }

    pub fn current(&self) -> Option<&NavigationEntry> {
        self.cursor.and_then(|cursor| self.entries.get(cursor))
    }

    pub fn entries(&self) -> &[NavigationEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(index: usize) -> NavigationEntry {
        NavigationEntry {
            project_id: format!("project-{index}"),
            worktree_id: format!("worktree-{index}"),
            area_id: format!("area-{index}"),
            tab_id: Some(format!("tab-{index}")),
        }
    }

    #[test]
    fn records_cap_duplicates_and_forward_truncation() {
        let mut history = NavigationHistory::default();
        for index in 0..=100 {
            history.record(entry(index));
        }
        assert_eq!(history.len(), 100);
        assert_eq!(history.current(), Some(&entry(100)));
        history.record(entry(100));
        assert_eq!(history.len(), 100);

        let target = history.target(Direction::Back, |_| true).unwrap();
        assert_eq!(target.entry, entry(99));
        assert!(history.commit_target(target.index));
        history.record(entry(200));
        assert_eq!(history.current(), Some(&entry(200)));
        assert!(!history.can_navigate(Direction::Forward, |_| true));
    }

    #[test]
    fn targets_skip_dead_entries_without_moving_until_commit() {
        let mut history = NavigationHistory::default();
        history.record(entry(0));
        history.record(entry(1));
        history.record(entry(2));
        let target = history
            .target(Direction::Back, |candidate| candidate != &entry(1))
            .unwrap();
        assert_eq!(target.entry, entry(0));
        assert_eq!(history.current(), Some(&entry(2)));
        assert!(history.commit_target(target.index));
        assert_eq!(history.current(), Some(&entry(0)));
    }

    #[test]
    fn prune_removes_dead_entries_and_preserves_logical_cursor() {
        let mut history = NavigationHistory::default();
        for index in 0..5 {
            history.record(entry(index));
        }
        let target = history.target(Direction::Back, |_| true).unwrap();
        history.commit_target(target.index);
        history.prune(|candidate| candidate != &entry(1) && candidate != &entry(4));
        assert_eq!(history.entries(), &[entry(0), entry(2), entry(3)]);
        assert_eq!(history.current(), Some(&entry(3)));
        assert!(history.can_navigate(Direction::Back, |_| true));
        assert!(!history.can_navigate(Direction::Forward, |_| true));
    }
}
