use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

const MAX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_KEY_CHARACTERS: usize = 256;
const MAX_VALUE_BYTES: usize = 1_048_576;
const MAX_STORE_BYTES: usize = 5_242_880;

pub(super) fn read(path: &Path) -> io::Result<Value> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(io::Error::other("extension data exceeds 8 MiB"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

pub(super) fn write(path: &Path, value: &Value) -> Result<(), String> {
    let write = || -> io::Result<()> {
        let bytes = serde_json::to_vec(value)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(io::Error::other("extension data exceeds 8 MiB"));
        }
        let directory = path
            .parent()
            .ok_or_else(|| io::Error::other("invalid data path"))?;
        std::fs::create_dir_all(directory)?;
        let temp = directory.join(format!(".extension-{}", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&temp, path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    };
    write().map_err(|error| error.to_string())
}

fn object(path: &Path) -> Result<Map<String, Value>, String> {
    match read(path) {
        Ok(value) => serde_json::from_value(value).map_err(|e| e.to_string()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Map::new()),
        Err(error) => Err(error.to_string()),
    }
}

/// The per-extension key/value store behind `muxy.storage`.
#[derive(Debug)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(profile: &Path) -> Self {
        Self {
            root: profile.join("extension-storage"),
        }
    }

    pub fn call(
        &self,
        extension: &super::Extension,
        verb: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let path = self.root.join(format!("{}.json", extension.name));
        let mut values = object(&path)?;
        if verb == "storage.keys" {
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort();
            return Ok(serde_json::json!(keys));
        }
        let key = args["key"].as_str().ok_or("missing argument 'key'")?;
        if key.is_empty() {
            return Err("storage key must not be empty".into());
        }
        if key.chars().count() > MAX_KEY_CHARACTERS {
            return Err(format!(
                "storage key exceeds {MAX_KEY_CHARACTERS} characters"
            ));
        }
        match verb {
            "storage.get" => return Ok(values.get(key).cloned().unwrap_or(Value::Null)),
            "storage.set" => {
                let value = args
                    .get("value")
                    .ok_or("storage.set requires a value")?
                    .clone();
                if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > MAX_VALUE_BYTES {
                    return Err(format!("storage value exceeds {MAX_VALUE_BYTES} bytes"));
                }
                values.insert(key.into(), value);
            }
            "storage.delete" => {
                values.remove(key);
            }
            _ => return Err("unsupported storage operation".into()),
        }
        let value = Value::Object(values);
        if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > MAX_STORE_BYTES {
            return Err(format!(
                "storage for this extension exceeds {MAX_STORE_BYTES} bytes"
            ));
        }
        write(&path, &value).map(|()| Value::Null)
    }
}

/// Values users set for an extension's declared settings. They outlive
/// disabling and uninstalling so a reinstalled extension keeps its configuration.
#[derive(Debug)]
pub struct Settings {
    root: PathBuf,
}

impl Settings {
    pub fn new(profile: &Path) -> Self {
        Self {
            root: profile.join("extension-settings"),
        }
    }

    fn path(&self, extension: &str) -> PathBuf {
        self.root.join(format!("{extension}.json"))
    }

    /// The overrides stored for one extension.
    pub fn values(&self, extension: &str) -> Result<Map<String, Value>, String> {
        object(&self.path(extension))
    }

    /// Stores an override, or removes it so the manifest default applies again.
    pub fn set(
        &self,
        extension: &super::Extension,
        key: &str,
        value: Option<Value>,
    ) -> Result<Map<String, Value>, String> {
        if extension.manifest.setting(key).is_none() {
            return Err(format!("setting '{key}' not declared in manifest"));
        }
        let mut values = self.values(&extension.name)?;
        match value {
            Some(value) => {
                if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > 65_536 {
                    return Err("value exceeds 65536-byte limit".into());
                }
                values.insert(key.into(), value);
            }
            None => {
                values.remove(key);
            }
        }
        write(&self.path(&extension.name), &Value::Object(values.clone()))?;
        Ok(values)
    }
}
