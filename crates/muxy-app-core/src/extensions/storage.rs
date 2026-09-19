use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

const MAX_BYTES: u64 = 8 * 1024 * 1024;

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
        let mut values: Map<String, Value> = match read(&path) {
            Ok(value) => serde_json::from_value(value).map_err(|e| e.to_string())?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Map::new(),
            Err(error) => return Err(error.to_string()),
        };
        if verb == "storage.keys" {
            return Ok(serde_json::json!(values.keys().collect::<Vec<_>>()));
        }
        let key = args["key"]
            .as_str()
            .filter(|k| !k.is_empty() && k.len() <= 1024)
            .ok_or("invalid storage key")?;
        match verb {
            "storage.get" => return Ok(values.get(key).cloned().unwrap_or(Value::Null)),
            "storage.set" => {
                values.insert(key.into(), args["value"].clone());
            }
            "storage.delete" => {
                values.remove(key);
            }
            _ => return Err("unsupported storage operation".into()),
        }
        write(&path, &Value::Object(values))?;
        Ok(Value::Null)
    }
}
