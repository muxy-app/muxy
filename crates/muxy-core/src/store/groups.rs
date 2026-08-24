use crate::prefs::app_support_dir;
use crate::store::{HOME_PROJECT_ID, persistence};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

const ALL_PROJECTS: &str = "All Projects";

#[derive(Debug, Clone)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub project_ids: Vec<String>,
    pub is_local: bool,
    raw: Map<String, Value>,
}

#[derive(Debug, Clone)]
pub struct Groups {
    path: PathBuf,
    groups: Vec<Group>,
}

fn groups_path() -> PathBuf {
    app_support_dir().join("project-groups.json")
}

fn decode(value: &Value) -> Option<Group> {
    let raw = value.as_object()?;
    let id = raw.get("id")?.as_str()?.to_owned();
    let name = raw.get("name")?.as_str()?.to_owned();
    let project_ids = raw
        .get("projectIDs")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let is_local = raw
        .get("type")
        .and_then(Value::as_str)
        .is_none_or(|kind| kind == "local");
    Some(Group {
        id,
        name,
        sort_order: raw.get("sortOrder").and_then(Value::as_i64).unwrap_or(0),
        project_ids,
        is_local,
        raw: raw.clone(),
    })
}

impl Groups {
    pub fn load() -> Self {
        Self::load_from(groups_path())
    }

    pub fn load_from(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut groups: Vec<Group> = std::fs::read(&path)
            .ok()
            .and_then(|contents| serde_json::from_slice::<Value>(&contents).ok())
            .and_then(|value| match value {
                Value::Array(entries) => Some(entries),
                _ => None,
            })
            .map(|entries| entries.iter().filter_map(decode).collect())
            .unwrap_or_default();
        groups.sort_by_key(|group| group.sort_order);
        Self { path, groups }
    }

    pub fn all(&self) -> &[Group] {
        &self.groups
    }

    pub fn group_id_containing(&self, project_id: &str) -> Option<&str> {
        self.groups
            .iter()
            .find(|group| {
                group
                    .project_ids
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(project_id))
            })
            .map(|group| group.id.as_str())
    }

    pub fn name_for(&self, id: Option<&str>) -> String {
        id.and_then(|id| self.find(id))
            .map(|group| group.name.clone())
            .unwrap_or_else(|| ALL_PROJECTS.to_owned())
    }

    pub fn is_local(&self, id: &str) -> bool {
        self.find(id).is_some_and(|group| group.is_local)
    }

    fn find(&self, id: &str) -> Option<&Group> {
        self.groups
            .iter()
            .find(|group| group.id.eq_ignore_ascii_case(id))
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.groups
            .iter()
            .position(|group| group.id.eq_ignore_ascii_case(id))
    }

    pub fn add(&mut self, name: String) -> String {
        let id = crate::store::new_uuid();
        self.groups.push(Group {
            id: id.clone(),
            name,
            sort_order: self.groups.len() as i64,
            project_ids: Vec::new(),
            is_local: true,
            raw: Map::new(),
        });
        self.save();
        id
    }

    pub fn rename(&mut self, id: &str, name: String) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        self.groups[index].name = name;
        self.save();
    }

    pub fn remove(&mut self, id: &str) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        self.groups.remove(index);
        self.save();
    }

    pub fn add_project(&mut self, project_id: &str, group_id: &str) -> bool {
        if project_id.eq_ignore_ascii_case(HOME_PROJECT_ID) {
            return false;
        }
        let Some(index) = self.index_of(group_id) else {
            return false;
        };
        if !self.groups[index].is_local {
            return false;
        }
        for (position, group) in self.groups.iter_mut().enumerate() {
            if position != index {
                group
                    .project_ids
                    .retain(|candidate| !candidate.eq_ignore_ascii_case(project_id));
            }
        }
        if !self.groups[index]
            .project_ids
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(project_id))
        {
            self.groups[index].project_ids.push(project_id.to_owned());
        }
        self.save();
        true
    }

    pub fn remove_project(&mut self, project_id: &str, group_id: &str) {
        let Some(index) = self.index_of(group_id) else {
            return;
        };
        self.groups[index]
            .project_ids
            .retain(|candidate| !candidate.eq_ignore_ascii_case(project_id));
        self.save();
    }

    pub fn remove_project_everywhere(&mut self, project_id: &str) {
        for group in &mut self.groups {
            group
                .project_ids
                .retain(|candidate| !candidate.eq_ignore_ascii_case(project_id));
        }
        self.save();
    }

    fn save(&self) {
        let encoded: Vec<Value> = self.groups.iter().map(encode).collect();
        let _ = persistence::write_json(&self.path, &encoded);
    }
}

fn encode(group: &Group) -> Value {
    let mut raw = group.raw.clone();
    raw.insert("id".to_owned(), Value::String(group.id.clone()));
    raw.insert("name".to_owned(), Value::String(group.name.clone()));
    raw.insert("sortOrder".to_owned(), Value::from(group.sort_order));
    raw.insert(
        "projectIDs".to_owned(),
        Value::Array(
            group
                .project_ids
                .iter()
                .map(|id| Value::String(id.clone()))
                .collect(),
        ),
    );
    if !raw.contains_key("type") {
        raw.insert("type".to_owned(), Value::String("local".to_owned()));
    }
    if !raw.contains_key("remoteProjects") {
        raw.insert("remoteProjects".to_owned(), Value::Array(Vec::new()));
    }
    Value::Object(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_path(label: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "muxy-groups-{}-{label}-{}",
            std::process::id(),
            crate::store::new_uuid()
        ));
        std::fs::create_dir_all(&directory).expect("temp dir");
        directory.join("project-groups.json")
    }

    fn write(path: &Path, value: &Value) {
        std::fs::write(path, serde_json::to_vec(value).expect("encode")).expect("write");
    }

    fn read(path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).expect("read")).expect("decode")
    }

    fn assert_keeps_original_keys(original: &Value, saved: &Value) {
        let original = original.as_array().expect("array");
        let saved = saved.as_array().expect("array");
        assert_eq!(original.len(), saved.len());
        for before in original {
            let before = before.as_object().expect("object");
            let id = before.get("id").expect("id");
            let after = saved
                .iter()
                .filter_map(Value::as_object)
                .find(|candidate| candidate.get("id") == Some(id))
                .unwrap_or_else(|| panic!("group {id} was dropped"));
            for (key, value) in before {
                assert_eq!(after.get(key), Some(value), "key {key} changed");
            }
        }
    }

    #[test]
    fn preserves_ssh_data_and_unknown_keys_through_a_round_trip() {
        let path = temp_path("ssh");
        let original = json!([
            {
                "id": "GROUP-SSH",
                "name": "Remote",
                "sortOrder": 0,
                "projectIDs": [],
                "type": "ssh",
                "remoteDeviceID": "DEVICE-ID",
                "remoteProjects": [],
                "sshData": {"host": "example.com"},
                "futureKey": 42
            }
        ]);
        write(&path, &original);

        let groups = Groups::load_from(&path);
        groups.save();

        assert_keeps_original_keys(&original, &read(&path));
    }

    #[test]
    fn inserts_remote_projects_for_a_legacy_group_without_it() {
        let path = temp_path("legacy");
        let original = json!([
            {"id": "GROUP-A", "name": "Work", "sortOrder": 0, "projectIDs": []}
        ]);
        write(&path, &original);

        let groups = Groups::load_from(&path);
        groups.save();

        let saved = read(&path);
        assert_keeps_original_keys(&original, &saved);
        assert_eq!(saved[0]["remoteProjects"], json!([]));
        assert_eq!(saved[0]["type"], json!("local"));
    }

    #[test]
    fn add_project_moves_it_out_of_its_previous_group() {
        let path = temp_path("move");
        write(
            &path,
            &json!([
                {"id": "GROUP-A", "name": "A", "sortOrder": 0, "projectIDs": ["PROJECT-1"]},
                {"id": "GROUP-B", "name": "B", "sortOrder": 1, "projectIDs": []}
            ]),
        );
        let mut groups = Groups::load_from(&path);

        assert!(groups.add_project("PROJECT-1", "GROUP-B"));

        assert!(groups.all()[0].project_ids.is_empty());
        assert_eq!(groups.all()[1].project_ids, vec!["PROJECT-1".to_owned()]);
        assert_eq!(groups.group_id_containing("PROJECT-1"), Some("GROUP-B"));
    }

    #[test]
    fn add_project_refuses_the_home_project_and_non_local_groups() {
        let path = temp_path("refuse");
        write(
            &path,
            &json!([
                {"id": "GROUP-A", "name": "A", "sortOrder": 0, "projectIDs": []},
                {"id": "GROUP-SSH", "name": "Remote", "sortOrder": 1, "projectIDs": [], "type": "ssh"}
            ]),
        );
        let mut groups = Groups::load_from(&path);

        assert!(!groups.add_project(HOME_PROJECT_ID, "GROUP-A"));
        assert!(!groups.add_project("PROJECT-1", "GROUP-SSH"));
        assert!(groups.add_project("PROJECT-1", "GROUP-A"));
    }

    #[test]
    fn name_for_falls_back_to_all_projects() {
        let path = temp_path("name");
        write(
            &path,
            &json!([{"id": "GROUP-A", "name": "Work", "sortOrder": 0, "projectIDs": []}]),
        );
        let groups = Groups::load_from(&path);

        assert_eq!(groups.name_for(Some("GROUP-A")), "Work");
        assert_eq!(groups.name_for(Some("MISSING")), "All Projects");
        assert_eq!(groups.name_for(None), "All Projects");
    }

    #[test]
    fn round_trips_a_file_whose_order_differs_from_sort_order() {
        let path = temp_path("unsorted");
        let original = json!([
            {"id": "GROUP-B", "name": "B", "sortOrder": 4, "projectIDs": [], "type": "local", "remoteProjects": []},
            {"id": "GROUP-A", "name": "A", "sortOrder": 1, "projectIDs": [], "type": "local", "remoteProjects": []}
        ]);
        write(&path, &original);

        let groups = Groups::load_from(&path);
        groups.save();

        assert_keeps_original_keys(&original, &read(&path));
        assert_eq!(groups.all()[0].id, "GROUP-A");
    }

    #[test]
    fn round_trips_the_live_project_groups_file() {
        let path = groups_path();
        let Ok(contents) = std::fs::read(&path) else {
            return;
        };
        let Ok(original) = serde_json::from_slice::<Value>(&contents) else {
            return;
        };
        if !original.is_array() {
            return;
        }

        let groups = Groups::load_from(&path);
        let encoded = Value::Array(groups.all().iter().map(encode).collect());

        assert_keeps_original_keys(&original, &encoded);
    }
}
