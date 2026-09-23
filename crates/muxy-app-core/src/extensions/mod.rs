mod manifest;
mod permissions;
mod storage;

pub mod api;
pub use manifest::{
    Action, BarItem, Command, Extension, FileOpener, HomeView, Icon, Localization, Manifest,
    PERMISSIONS, Panel, PanelControl, PanelMode, PanelPosition, Popover, RemoteMethod, Setting,
    SettingKind, Side, Sidebar, StatusBarItem, TabType, local_event,
};
pub use permissions::{
    Choice, Consent, Gate, Grants, Request, event_permission, required_permission,
};
pub use storage::{Settings, Storage};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Installed packages and explicit developer folders for this app profile.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub extensions: BTreeMap<String, Extension>,
    pub errors: Vec<String>,
    enabled: BTreeSet<String>,
    unpacked: BTreeMap<String, PathBuf>,
    profile: PathBuf,
}

impl Registry {
    pub fn empty(profile: &Path) -> Self {
        Self {
            profile: profile.into(),
            ..Self::default()
        }
    }
    pub fn load(profile: &Path) -> Self {
        let mut registry = Self {
            profile: profile.into(),
            ..Self::default()
        };
        registry.enabled = registry.read_preference("extension-enabled.json");
        registry.unpacked = registry.read_preference("extension-folders.json");
        registry.reload();
        registry
    }

    fn read_preference<T: serde::de::DeserializeOwned + Default>(&mut self, name: &str) -> T {
        match storage::read(&self.profile.join(name)) {
            Ok(value) => match serde_json::from_value(value) {
                Ok(value) => value,
                Err(error) => {
                    self.errors.push(format!("{name}: {error}"));
                    T::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => T::default(),
            Err(error) => {
                self.errors.push(format!("{name}: {error}"));
                T::default()
            }
        }
    }

    pub fn directory(&self) -> PathBuf {
        self.profile.join("extensions")
    }

    pub fn reload(&mut self) {
        self.extensions.clear();
        match std::fs::read_dir(self.directory()) {
            Ok(entries) => {
                for entry in entries {
                    let result = entry.map_err(|e| e.to_string()).and_then(|entry| {
                        if entry.file_name().as_encoded_bytes().starts_with(b".")
                            || !entry.path().is_dir()
                        {
                            return Ok(());
                        }
                        let extension = Extension::load(&entry.path())?;
                        if entry.file_name() != std::ffi::OsStr::new(&extension.name) {
                            return Err(
                                "installed extension name does not match its directory".into()
                            );
                        }
                        self.extensions.insert(extension.name.clone(), extension);
                        Ok(())
                    });
                    if let Err(error) = result {
                        self.errors.push(error);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => self.errors.push(error.to_string()),
        }
        for (name, path) in &self.unpacked {
            match Self::unpacked_extension(path) {
                Ok(extension) if extension.name == *name && !self.extensions.contains_key(name) => {
                    self.extensions.insert(name.clone(), extension);
                }
                Ok(_) => self
                    .errors
                    .push(format!("{name}: duplicate or changed package name")),
                Err(error) => self.errors.push(error),
            }
        }
    }

    pub fn unpacked_extension(path: &Path) -> Result<Extension, String> {
        Extension::load(path)
    }

    pub fn load_unpacked(&mut self, path: &Path) -> Result<String, String> {
        let extension = Self::unpacked_extension(path)?;
        if self.extensions.contains_key(&extension.name) {
            return Err("an extension with this name is already loaded".into());
        }
        let mut folders = self.unpacked.clone();
        folders.insert(
            extension.name.clone(),
            path.canonicalize().map_err(|e| e.to_string())?,
        );
        storage::write(
            &self.profile.join("extension-folders.json"),
            &serde_json::json!(folders),
        )?;
        self.unpacked = folders;
        let name = extension.name.clone();
        self.extensions.insert(name.clone(), extension);
        Ok(name)
    }

    pub fn is_unpacked(&self, id: &str) -> bool {
        self.unpacked.contains_key(id)
    }

    pub fn enabled(&self, id: &str) -> Option<&Extension> {
        self.extensions
            .get(id)
            .filter(|_| self.enabled.contains(id))
    }

    pub fn active(&self) -> impl Iterator<Item = &Extension> {
        self.extensions
            .values()
            .filter(|extension| self.enabled.contains(&extension.name))
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        if !self.extensions.contains_key(id) {
            return Err("extension is not installed".into());
        }
        let mut active = self.enabled.clone();
        if enabled {
            active.insert(id.into());
        } else {
            active.remove(id);
        }
        storage::write(
            &self.profile.join("extension-enabled.json"),
            &serde_json::json!(active),
        )?;
        self.enabled = active;
        Ok(())
    }

    /// Forget developer folders without deleting their contents.
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        self.set_enabled(id, false)?;
        if self.is_unpacked(id) {
            let mut folders = self.unpacked.clone();
            folders.remove(id);
            storage::write(
                &self.profile.join("extension-folders.json"),
                &serde_json::json!(folders),
            )?;
            self.unpacked = folders;
        } else {
            std::fs::remove_dir_all(self.directory().join(id)).map_err(|e| e.to_string())?;
        }
        self.extensions.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
