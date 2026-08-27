use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "preferences.json";

fn path() -> PathBuf {
    super::app_support_dir().join(FILE_NAME)
}

fn read_map(path: &Path) -> std::io::Result<Map<String, Value>> {
    let contents = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(error),
    };
    let value = serde_json::from_slice::<Value>(&contents)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "expected JSON object"))
}

fn read_current() -> Option<Map<String, Value>> {
    let path = path();
    match read_map(&path) {
        Ok(values) => Some(values),
        Err(error) => {
            log::warn!("failed to read {}: {error}", path.display());
            None
        }
    }
}

fn write_map(path: &Path, values: &Map<String, Value>) -> std::io::Result<()> {
    let contents = serde_json::to_vec_pretty(values)?;
    crate::store::write_private(path, &contents)
}

fn update(key: &str, value: Option<Value>) {
    let path = path();
    if let Err(error) = update_at(&path, key, value) {
        log::warn!("failed to update {}: {error}", path.display());
    }
}

fn update_at(path: &Path, key: &str, value: Option<Value>) -> std::io::Result<()> {
    let mut values = read_map(path)?;
    match value {
        Some(value) => {
            values.insert(key.to_owned(), value);
        }
        None => {
            values.remove(key);
        }
    }
    write_map(path, &values)
}

pub fn store_bool(key: &str, value: bool) {
    update(key, Some(Value::Bool(value)));
}

pub fn store_string(key: &str, value: Option<&str>) {
    update(key, value.map(|value| Value::String(value.to_owned())));
}

pub fn store_i64(key: &str, value: i64) {
    update(key, Some(Value::Number(Number::from(value))));
}

pub fn store_f64(key: &str, value: f64) {
    if let Some(value) = Number::from_f64(value) {
        update(key, Some(Value::Number(value)));
    }
}

pub fn store_dictionary(key: &str, value: &HashMap<String, String>) {
    let value = value
        .iter()
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect();
    update(key, Some(Value::Object(value)));
}

pub fn try_store_dictionary(key: &str, value: &HashMap<String, String>) -> std::io::Result<()> {
    let value = value
        .iter()
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect();
    update_at(&path(), key, Some(Value::Object(value)))
}

pub fn remove(key: &str) {
    update(key, None);
}

pub fn read_bool(key: &str) -> Option<bool> {
    read_current()?.get(key).and_then(Value::as_bool)
}

pub fn read_i64(key: &str) -> Option<i64> {
    read_current()?.get(key).and_then(Value::as_i64)
}

pub fn read_f64(key: &str) -> Option<f64> {
    read_current()?.get(key).and_then(Value::as_f64)
}

pub fn read_string(key: &str) -> Option<String> {
    read_current()?
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

pub fn read_dictionary(key: &str) -> Option<HashMap<String, String>> {
    read_current()?
        .get(key)
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                .collect()
        })
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn merge_imported(root: &Path, imported: &Map<String, Value>) -> std::io::Result<()> {
    let path = root.join(FILE_NAME);
    let mut values = read_map(&path)?;
    for (key, value) in imported {
        values.entry(key.clone()).or_insert_with(|| value.clone());
    }
    write_map(&path, &values)
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value};
    use std::collections::HashMap;

    #[test]
    fn portable_preferences_round_trip_every_supported_type() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("preferences.json");
        let mut values = Map::new();
        values.insert("bool".to_owned(), Value::Bool(true));
        values.insert("integer".to_owned(), Value::from(1800));
        values.insert("double".to_owned(), Value::from(1.25));
        values.insert("string".to_owned(), Value::String("hello".to_owned()));
        values.insert(
            "dictionary".to_owned(),
            serde_json::json!({ "alpha": "one" }),
        );
        super::write_map(&path, &values).expect("write");

        let stored = super::read_map(&path).expect("read");
        assert_eq!(stored.get("bool").and_then(Value::as_bool), Some(true));
        assert_eq!(stored.get("integer").and_then(Value::as_i64), Some(1800));
        assert_eq!(stored.get("double").and_then(Value::as_f64), Some(1.25));
        assert_eq!(stored.get("string").and_then(Value::as_str), Some("hello"));
        assert_eq!(
            stored
                .get("dictionary")
                .and_then(Value::as_object)
                .and_then(|value| value.get("alpha"))
                .and_then(Value::as_str),
            Some("one")
        );
    }

    #[test]
    fn imported_preferences_never_replace_existing_values() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("preferences.json");
        let existing = serde_json::json!({ "shared": "rust", "existing": true });
        super::write_map(&path, existing.as_object().expect("object")).expect("write");
        let imported = serde_json::json!({ "shared": "swift", "missing": 42 });
        super::merge_imported(directory.path(), imported.as_object().expect("object"))
            .expect("merge");
        assert_eq!(
            super::read_map(&path).expect("read"),
            serde_json::json!({ "shared": "rust", "existing": true, "missing": 42 })
                .as_object()
                .expect("object")
                .clone()
        );
    }

    #[test]
    fn ordinary_updates_do_not_replace_a_malformed_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("preferences.json");
        std::fs::write(&path, b"not json").expect("write malformed");
        let result = super::update_at(&path, "new", Some(Value::Bool(true)));
        assert_eq!(
            result.expect_err("malformed preferences").kind(),
            std::io::ErrorKind::InvalidData
        );
        assert_eq!(std::fs::read(&path).expect("unchanged"), b"not json");
    }

    #[test]
    fn imported_preferences_do_not_replace_a_malformed_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("preferences.json");
        std::fs::write(&path, b"not json").expect("write malformed");
        let imported = serde_json::json!({ "imported": true });
        let result = super::merge_imported(directory.path(), imported.as_object().expect("object"));
        assert_eq!(
            result.expect_err("malformed preferences").kind(),
            std::io::ErrorKind::InvalidData
        );
        assert_eq!(std::fs::read(&path).expect("unchanged"), b"not json");
    }

    #[test]
    fn dictionary_values_are_string_maps() {
        let value: HashMap<String, String> = [
            ("alpha".to_owned(), "one".to_owned()),
            ("beta".to_owned(), "two".to_owned()),
        ]
        .into_iter()
        .collect();
        let json: Value = serde_json::to_value(&value).expect("serialize");
        assert!(json.as_object().is_some());
    }
}
