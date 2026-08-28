use super::{NotificationRecord, NotificationTarget};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const NOTIFICATION_LIMIT: usize = 200;

#[derive(Debug)]
pub struct NotificationStore {
    path: PathBuf,
    records: Vec<NotificationRecord>,
    dirty_revision: u64,
    flushed_revision: u64,
}

impl NotificationStore {
    pub fn load() -> Self {
        Self::load_from(crate::prefs::app_support_dir().join("notifications.json"))
    }

    pub fn load_from(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let records = load_records(&path);
        Self {
            path,
            records,
            dirty_revision: 0,
            flushed_revision: 0,
        }
    }

    pub fn empty_at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            records: Vec::new(),
            dirty_revision: 0,
            flushed_revision: 0,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn records(&self) -> &[NotificationRecord] {
        &self.records
    }

    pub fn get(&self, id: &str) -> Option<&NotificationRecord> {
        self.records
            .iter()
            .find(|record| record.id.eq_ignore_ascii_case(id))
    }

    pub fn dirty_revision(&self) -> u64 {
        self.dirty_revision
    }

    pub fn needs_flush(&self) -> bool {
        self.dirty_revision != self.flushed_revision
    }

    pub fn insert(&mut self, record: NotificationRecord) {
        self.records.insert(0, record);
        self.records.truncate(NOTIFICATION_LIMIT);
        self.touch();
    }

    pub fn unread_total(&self) -> usize {
        self.records.iter().filter(|record| !record.is_read).count()
    }

    pub fn unread_project(&self, project_id: &str) -> usize {
        self.records
            .iter()
            .filter(|record| !record.is_read && record.project_id.eq_ignore_ascii_case(project_id))
            .count()
    }

    pub fn unread_worktree(&self, project_id: &str, worktree_id: &str) -> usize {
        self.records
            .iter()
            .filter(|record| {
                !record.is_read
                    && record.project_id.eq_ignore_ascii_case(project_id)
                    && record.worktree_id.eq_ignore_ascii_case(worktree_id)
            })
            .count()
    }

    pub fn unread_tab(&self, project_id: &str, worktree_id: &str, tab_id: &str) -> usize {
        self.records
            .iter()
            .filter(|record| {
                !record.is_read
                    && record.project_id.eq_ignore_ascii_case(project_id)
                    && record.worktree_id.eq_ignore_ascii_case(worktree_id)
                    && record.tab_id.eq_ignore_ascii_case(tab_id)
            })
            .count()
    }

    pub fn mark_read(&mut self, id: &str) -> bool {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.id.eq_ignore_ascii_case(id) && !record.is_read)
        else {
            return false;
        };
        record.is_read = true;
        self.touch();
        true
    }

    pub fn mark_tab_read(&mut self, project_id: &str, worktree_id: &str, tab_id: &str) -> bool {
        let mut changed = false;
        for record in &mut self.records {
            if !record.is_read
                && record.project_id.eq_ignore_ascii_case(project_id)
                && record.worktree_id.eq_ignore_ascii_case(worktree_id)
                && record.tab_id.eq_ignore_ascii_case(tab_id)
            {
                record.is_read = true;
                changed = true;
            }
        }
        self.finish_mutation(changed)
    }

    pub fn mark_all_read(&mut self) -> bool {
        let mut changed = false;
        for record in &mut self.records {
            if !record.is_read {
                record.is_read = true;
                changed = true;
            }
        }
        self.finish_mutation(changed)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let previous_len = self.records.len();
        self.records
            .retain(|record| !record.id.eq_ignore_ascii_case(id));
        self.finish_mutation(self.records.len() != previous_len)
    }

    pub fn clear(&mut self) -> bool {
        if self.records.is_empty() {
            return false;
        }
        self.records.clear();
        self.touch();
        true
    }

    pub fn flush(&mut self) -> std::io::Result<bool> {
        self.flush_if_revision(self.dirty_revision)
    }

    pub fn flush_if_revision(&mut self, revision: u64) -> std::io::Result<bool> {
        if revision != self.dirty_revision || !self.needs_flush() {
            return Ok(false);
        }
        let contents = serde_json::to_vec_pretty(&self.records).map_err(std::io::Error::other)?;
        crate::store::write_private(&self.path, &contents)?;
        self.flushed_revision = revision;
        Ok(true)
    }

    fn finish_mutation(&mut self, changed: bool) -> bool {
        if changed {
            self.touch();
        }
        changed
    }

    fn touch(&mut self) {
        self.dirty_revision = self.dirty_revision.saturating_add(1);
    }
}

fn load_records(path: &Path) -> Vec<NotificationRecord> {
    let Ok(contents) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Array(rows)) = serde_json::from_slice(&contents) else {
        return Vec::new();
    };
    let mut ids = HashSet::new();
    let mut records = Vec::new();
    for row in rows {
        let Ok(raw) = serde_json::from_value::<NotificationRecord>(row) else {
            continue;
        };
        let Some(target) = NotificationTarget::new(
            raw.pane_id,
            raw.project_id,
            raw.worktree_id,
            raw.area_id,
            raw.tab_id,
            raw.worktree_path,
        ) else {
            continue;
        };
        let Some(record) = NotificationRecord::with_id(
            raw.id,
            target,
            raw.source,
            raw.title,
            raw.body,
            raw.timestamp,
            raw.is_read,
        ) else {
            continue;
        };
        if !ids.insert(record.id.clone()) {
            continue;
        }
        records.push(record);
        if records.len() == NOTIFICATION_LIMIT {
            break;
        }
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::{NotificationSource, NotificationTarget};
    use std::fs;

    const PANE: &str = "11111111-2222-4333-8444-555555555555";
    const PROJECT: &str = "22222222-3333-4444-8555-666666666666";
    const WORKTREE: &str = "33333333-4444-4555-8666-777777777777";
    const AREA: &str = "44444444-5555-4666-8777-888888888888";
    const TAB: &str = "55555555-6666-4777-8888-999999999999";

    fn target(project: &str, worktree: &str, tab: &str) -> NotificationTarget {
        NotificationTarget::new(PANE, project, worktree, AREA, tab, "/tmp/worktree")
            .expect("target")
    }

    fn record(index: usize) -> NotificationRecord {
        NotificationRecord::with_id(
            format!("00000000-0000-4000-8000-{index:012X}"),
            target(PROJECT, WORKTREE, TAB),
            NotificationSource::AiProvider {
                provider_id: "codex".to_owned(),
            },
            format!("Title {index}"),
            format!("Body {index}"),
            index as f64,
            false,
        )
        .expect("record")
    }

    fn row(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "paneID": PANE,
            "projectID": PROJECT,
            "worktreeID": WORKTREE,
            "areaID": AREA,
            "tabID": TAB,
            "worktreePath": "/tmp/worktree",
            "source": {"type": "aiProvider", "providerID": "codex"},
            "title": "Task completed!",
            "body": "Finished",
            "timestamp": 796000000.0,
            "isRead": false
        })
    }

    #[test]
    fn notifications_load_missing_invalid_and_non_array_as_empty_without_rewrite() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("missing.json");
        assert!(NotificationStore::load_from(&missing).records().is_empty());
        assert!(!missing.exists());

        for contents in ["not-json", "{}"] {
            let path = dir.path().join(format!("fixture-{}.json", contents.len()));
            fs::write(&path, contents).expect("write fixture");
            let before = fs::read(&path).expect("before");
            assert!(NotificationStore::load_from(&path).records().is_empty());
            assert_eq!(fs::read(&path).expect("after"), before);
        }
    }

    #[test]
    fn notifications_load_rows_independently_and_apply_shape_rules() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("notifications.json");
        let mut first = row("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee");
        first.as_object_mut().expect("object").remove("isRead");
        first["unknown"] = serde_json::json!(true);
        let duplicate = row("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE");
        let malformed_id = row("bad");
        let unsupported_source = {
            let mut value = row("BBBBBBBB-BBBB-4CCC-8DDD-EEEEEEEEEEEE");
            value["source"] = serde_json::json!({"type": "future"});
            value
        };
        let malformed_timestamp = {
            let mut value = row("CCCCCCCC-BBBB-4CCC-8DDD-EEEEEEEEEEEE");
            value["timestamp"] = serde_json::json!("NaN");
            value
        };
        let mut second = row("DDDDDDDD-BBBB-4CCC-8DDD-EEEEEEEEEEEE");
        second["title"] = serde_json::json!("");
        second["body"] = serde_json::json!("");
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!([
                first,
                {"broken": true},
                duplicate,
                malformed_id,
                unsupported_source,
                malformed_timestamp,
                second
            ]))
            .expect("fixture"),
        )
        .expect("write");

        let store = NotificationStore::load_from(&path);
        assert_eq!(store.records().len(), 2);
        assert_eq!(
            store.records()[0].id,
            "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"
        );
        assert!(!store.records()[0].is_read);
        assert_eq!(
            store.records()[1].id,
            "DDDDDDDD-BBBB-4CCC-8DDD-EEEEEEEEEEEE"
        );
        assert_eq!(store.records()[1].title, "");
        assert_eq!(store.records()[1].body, "");
        assert!(!store.needs_flush());
    }

    #[test]
    fn notifications_store_serializes_the_top_level_array_snapshot() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("notifications.json");
        let mut store = NotificationStore::empty_at(&path);
        let expected = record(1);
        store.insert(expected.clone());
        assert!(store.flush().expect("flush"));
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read")).expect("JSON");
        assert_eq!(value, serde_json::json!([expected]));
        assert!(value.is_array());
    }

    #[test]
    fn notifications_mutations_queries_and_dirty_revisions_are_exact() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = NotificationStore::empty_at(dir.path().join("notifications.json"));
        let first = record(1);
        let second_project = "66666666-7777-4888-8999-AAAAAAAAAAAA";
        let second_worktree = "77777777-8888-4999-8AAA-BBBBBBBBBBBB";
        let second_tab = "88888888-9999-4AAA-8BBB-CCCCCCCCCCCC";
        let second = NotificationRecord::with_id(
            "99999999-AAAA-4BBB-8CCC-DDDDDDDDDDDD",
            target(second_project, second_worktree, second_tab),
            NotificationSource::Socket,
            "Second",
            "",
            2.0,
            false,
        )
        .expect("record");
        store.insert(first.clone());
        store.insert(second.clone());
        assert_eq!(store.records()[0].id, second.id);
        assert_eq!(store.unread_total(), 2);
        assert_eq!(store.unread_project(PROJECT), 1);
        assert_eq!(store.unread_project(second_project), 1);
        assert_eq!(store.unread_worktree(PROJECT, WORKTREE), 1);
        assert_eq!(store.unread_tab(PROJECT, WORKTREE, TAB), 1);

        let revision = store.dirty_revision();
        assert!(!store.mark_read("missing"));
        assert_eq!(store.dirty_revision(), revision);
        assert!(store.mark_read(&first.id));
        assert!(!store.mark_read(&first.id));
        assert_eq!(store.unread_total(), 1);
        assert!(store.mark_tab_read(second_project, second_worktree, second_tab));
        assert!(!store.mark_tab_read(second_project, second_worktree, second_tab));
        assert_eq!(store.unread_total(), 0);
        assert!(!store.mark_all_read());
        assert!(store.remove(&first.id));
        assert!(!store.remove(&first.id));
        assert!(store.clear());
        assert!(!store.clear());
    }

    #[test]
    fn notifications_insert_and_load_cap_at_two_hundred_newest_first() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("notifications.json");
        let mut store = NotificationStore::empty_at(&path);
        for index in 0..205 {
            store.insert(record(index));
        }
        assert_eq!(store.records().len(), NOTIFICATION_LIMIT);
        assert_eq!(store.records()[0].title, "Title 204");
        assert_eq!(store.records()[199].title, "Title 5");
        store.flush().expect("flush");

        let loaded = NotificationStore::load_from(&path);
        assert_eq!(loaded.records().len(), NOTIFICATION_LIMIT);
        assert_eq!(loaded.records()[0].title, "Title 204");
        assert_eq!(loaded.records()[199].title, "Title 5");

        let rows = (0..205).map(record).collect::<Vec<_>>();
        fs::write(&path, serde_json::to_vec(&rows).expect("rows")).expect("write rows");
        let loaded = NotificationStore::load_from(&path);
        assert_eq!(loaded.records().len(), NOTIFICATION_LIMIT);
        assert_eq!(loaded.records()[0].title, "Title 0");
        assert_eq!(loaded.records()[199].title, "Title 199");
    }

    #[cfg(unix)]
    #[test]
    fn notifications_flush_is_private_atomic_and_revision_guarded() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("notifications.json");
        let mut store = NotificationStore::empty_at(&path);
        store.insert(record(1));
        let first_revision = store.dirty_revision();
        assert!(!store.flush_if_revision(first_revision + 1).expect("stale"));
        assert!(!path.exists());
        assert!(store.flush_if_revision(first_revision).expect("flush"));
        assert!(!store.needs_flush());
        assert!(!store.flush().expect("clean flush"));
        assert_eq!(
            fs::metadata(&path).expect("metadata").permissions().mode() & 0o777,
            0o600
        );
        let temporary_files = fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(temporary_files, 0);
    }
}
