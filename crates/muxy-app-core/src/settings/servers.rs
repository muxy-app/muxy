use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};

use super::{Error, Result, Settings};
use crate::ServerId;

/// Another computer whose Muxy server this app reaches over SSH.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerEntry {
    /// Any UUID spelling is read, such as `uuidgen` output.
    #[serde(deserialize_with = "any_uuid")]
    pub id: ServerId,
    pub name: String,
    /// An SSH destination: a `~/.ssh/config` alias, `user@host`, or
    /// `ssh://user@host:port`. It is fully checked when connecting.
    pub ssh: String,
}

impl ServerEntry {
    pub fn new(name: String, ssh: String) -> Self {
        Self {
            id: ServerId::new(),
            name,
            ssh,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.is_local() {
            return Err(Error::new(
                "servers",
                "this computer's server ID cannot be listed",
            ));
        }
        if self.name.trim().is_empty()
            || self.name.len() > 256
            || self.name.chars().any(char::is_control)
        {
            return Err(Error::new(
                "servers",
                "enter a name of 1–256 bytes without control characters",
            ));
        }
        if self.ssh.trim().is_empty() || self.ssh.chars().any(char::is_control) {
            return Err(Error::new(
                "servers",
                "enter an SSH destination without control characters",
            ));
        }
        Ok(())
    }
}

// Each write changes one entry of the list as the file holds it now, so
// entries edited by hand since loading are kept.
impl Settings {
    pub fn server(&self, id: ServerId) -> Option<&ServerEntry> {
        self.servers.iter().find(|server| server.id == id)
    }

    pub fn add_server(&mut self, server: ServerEntry, path: &Path) -> Result<()> {
        self.servers = edit_servers(path, |servers| {
            servers.push(server);
            Ok(())
        })?;
        Ok(())
    }

    /// Replaces the entry that has `server`'s ID.
    pub fn update_server(&mut self, server: ServerEntry, path: &Path) -> Result<()> {
        self.servers = edit_servers(path, |servers| {
            let entry = servers
                .iter_mut()
                .find(|entry| entry.id == server.id)
                .ok_or_else(|| Error::new("servers", "this server is no longer listed"))?;
            *entry = server;
            Ok(())
        })?;
        Ok(())
    }

    pub fn remove_server(&mut self, id: ServerId, path: &Path) -> Result<()> {
        self.servers = edit_servers(path, |servers| {
            servers.retain(|server| server.id != id);
            Ok(())
        })?;
        Ok(())
    }

    pub(super) fn validate_servers(&self) -> Result<()> {
        validate(&self.servers)
    }
}

fn any_uuid<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<ServerId, D::Error> {
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

fn validate(servers: &[ServerEntry]) -> Result<()> {
    let mut ids = HashSet::new();
    for server in servers {
        server.validate()?;
        if !ids.insert(server.id) {
            return Err(Error::new("servers", "server IDs must be unique"));
        }
    }
    Ok(())
}

fn edit_servers(
    path: &Path,
    change: impl FnOnce(&mut Vec<ServerEntry>) -> Result<()>,
) -> Result<Vec<ServerEntry>> {
    let mut document =
        super::appearance::read_document(path).map_err(|error| Error::new("servers", error))?;
    let mut servers: Vec<ServerEntry> = document
        .remove("servers")
        .map(toml::Value::try_into)
        .transpose()
        .map_err(|error| Error::new("servers", error))?
        .unwrap_or_default();
    change(&mut servers)?;
    validate(&servers)?;
    // An empty list is left out, so `[[servers]]` can be added by hand.
    if !servers.is_empty() {
        let value =
            toml::Value::try_from(&servers).map_err(|error| Error::new("servers", error))?;
        document.insert("servers".into(), value);
    }
    super::appearance::write_document(path, &document)
        .map_err(|error| Error::new("servers", error))?;
    Ok(servers)
}
