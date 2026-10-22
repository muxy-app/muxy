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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ssh: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_command: Option<String>,
    /// The private key to log in with, instead of ssh's own choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_file: Option<String>,
    /// Logs in with a password, which the app asks for and never saves.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub password_login: bool,
    /// The group the app lists it under; `None` leaves it ungrouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl ServerEntry {
    pub fn new(name: String, ssh: String) -> Self {
        Self {
            id: ServerId::new(),
            name,
            ssh,
            ssh_command: None,
            identity_file: None,
            password_login: false,
            group: None,
        }
    }

    pub fn destination(&self) -> &str {
        self.ssh_command.as_deref().unwrap_or(&self.ssh)
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
        if let Some(command) = &self.ssh_command {
            if !self.ssh.is_empty() || self.identity_file.is_some() || self.password_login {
                return Err(Error::new(
                    "servers",
                    "custom commands cannot be combined with an SSH destination, identity file, or password login",
                ));
            }
            muxy_core::executable::command_arguments(command)
                .map_err(|error| Error::new("servers", error))?;
        } else if self.ssh.trim().is_empty() || self.ssh.chars().any(char::is_control) {
            return Err(Error::new(
                "servers",
                "enter an SSH destination without control characters",
            ));
        }
        if self
            .identity_file
            .as_ref()
            .is_some_and(|path| path.trim().is_empty() || path.chars().any(char::is_control))
        {
            return Err(Error::new(
                "servers",
                "enter an identity file path without control characters",
            ));
        }
        if self.group.as_ref().is_some_and(|group| {
            group.trim().is_empty() || group.len() > 256 || group.chars().any(char::is_control)
        }) {
            return Err(Error::new(
                "servers",
                "enter a group name of 1–256 bytes without control characters",
            ));
        }
        Ok(())
    }
}

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

    pub fn remove_servers(&mut self, ids: &[ServerId], path: &Path) -> Result<()> {
        self.servers = edit_servers(path, |servers| {
            servers.retain(|server| !ids.contains(&server.id));
            Ok(())
        })?;
        Ok(())
    }

    /// Moves the listed servers into `group`, or out of theirs, in their
    /// order, before `before` or at the end. Servers no longer listed are
    /// skipped.
    pub fn place_servers(
        &mut self,
        ids: &[ServerId],
        group: Option<&str>,
        before: Option<ServerId>,
        path: &Path,
    ) -> Result<()> {
        self.servers = edit_servers(path, |servers| {
            let (mut moved, mut kept): (Vec<_>, Vec<_>) = std::mem::take(servers)
                .into_iter()
                .partition(|server| ids.contains(&server.id));
            for server in &mut moved {
                server.group = group.map(str::to_owned);
            }
            let at = before
                .and_then(|before| kept.iter().position(|server| server.id == before))
                .unwrap_or(kept.len());
            kept.splice(at..at, moved);
            *servers = kept;
            Ok(())
        })?;
        Ok(())
    }

    /// Puts each listed server in `group`, or none, where it is. Servers no
    /// longer listed are skipped.
    pub fn set_servers_group(
        &mut self,
        ids: &[ServerId],
        group: Option<&str>,
        path: &Path,
    ) -> Result<()> {
        self.servers = edit_servers(path, |servers| {
            for server in servers.iter_mut().filter(|server| ids.contains(&server.id)) {
                server.group = group.map(str::to_owned);
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Every group a server is in, in the order they are listed.
    pub fn server_groups(&self) -> Vec<&str> {
        let mut groups = Vec::new();
        for group in self
            .servers
            .iter()
            .filter_map(|server| server.group.as_deref())
        {
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
        groups
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

/// Changes the list as the file holds it now, so entries edited by hand
/// since loading are kept. An empty list is left out, so `[[servers]]` can be
/// added by hand.
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
    if !servers.is_empty() {
        let value =
            toml::Value::try_from(&servers).map_err(|error| Error::new("servers", error))?;
        document.insert("servers".into(), value);
    }
    super::appearance::write_document(path, &document)
        .map_err(|error| Error::new("servers", error))?;
    Ok(servers)
}
