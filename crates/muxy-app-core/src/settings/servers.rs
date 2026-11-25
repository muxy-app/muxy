use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};

use super::{Error, Result, Settings};
use crate::ServerId;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerEntry {
    #[serde(deserialize_with = "any_uuid")]
    pub id: ServerId,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ssh: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_file: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub password_login: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paired: Option<PairedServer>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairedServer {
    pub hosts: Vec<String>,
    pub port: u16,
    pub fingerprint: String,
    pub device: muxy_protocol::DeviceId,
}

impl PairedServer {
    pub fn fingerprint(&self) -> Option<[u8; 32]> {
        muxy_core::hex::decode(&self.fingerprint)
    }

    pub fn has_certificate(&self, fingerprint: &[u8; 32]) -> bool {
        self.fingerprint().as_ref() == Some(fingerprint)
    }

    fn validate(&self) -> Result<()> {
        let invalid = || {
            Error::new(
                "servers",
                "a paired server needs its addresses, port, and fingerprint",
            )
        };
        if self.hosts.is_empty()
            || self.port < 1024
            || self.fingerprint().is_none()
            || muxy_protocol::validate_pairing_hosts(&self.hosts).is_err()
        {
            return Err(invalid());
        }
        Ok(())
    }
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
            paired: None,
        }
    }

    pub fn paired(name: String, paired: PairedServer) -> Self {
        Self {
            paired: Some(paired),
            ..Self::new(name, String::new())
        }
    }

    pub fn destination(&self) -> &str {
        match (&self.paired, &self.ssh_command) {
            (Some(paired), _) => paired.hosts.first().map_or("", String::as_str),
            (None, Some(command)) => command,
            (None, None) => &self.ssh,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.is_local() {
            return Err(Error::new(
                "servers",
                "this computer's server ID cannot be listed",
            ));
        }
        Self::validate_name(&self.name)?;
        if let Some(paired) = &self.paired {
            if !self.ssh.is_empty()
                || self.ssh_command.is_some()
                || self.identity_file.is_some()
                || self.password_login
            {
                return Err(Error::new(
                    "servers",
                    "a paired server cannot also have an SSH destination, command, identity file, or password login",
                ));
            }
            paired.validate()?;
        } else if let Some(command) = &self.ssh_command {
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
        if let Some(group) = &self.group {
            Self::validate_group(group)?;
        }
        Ok(())
    }

    pub fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            return Err(Error::new(
                "servers",
                "enter a name of 1–256 bytes without control characters",
            ));
        }
        Ok(())
    }

    pub fn validate_group(group: &str) -> Result<()> {
        if group.trim().is_empty() || group.len() > 256 || group.chars().any(char::is_control) {
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

fn edit_servers(
    path: &Path,
    change: impl FnOnce(&mut Vec<ServerEntry>) -> Result<()>,
) -> Result<Vec<ServerEntry>> {
    let _lock = super::document::lock(path).map_err(|error| Error::new("servers", error))?;
    let mut document =
        super::document::read_document(path).map_err(|error| Error::new("servers", error))?;
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
    let source = toml::to_string_pretty(&document).map_err(|error| Error::new("servers", error))?;
    super::document::atomic_write(path, &source).map_err(|error| Error::new("servers", error))?;
    Ok(servers)
}
