use gpui::{Context, Task};
use muxy_app_core::ServerId;
use muxy_app_core::settings::{PairedServer, PairingTokens, ServerEntry};
use muxy_client::{Client, ClientError};
use muxy_protocol::{DeviceKind, ErrorCode, Paired, PairingInvite};
use muxy_ui::tr;

use super::{AppModel, Quitting};

#[derive(Default)]
pub(crate) struct Pairing {
    running: Option<Task<()>>,
    unsaved: Option<(PairingInvite, Paired)>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PairingForm {
    pub(crate) link: String,
    pub(crate) name: String,
    pub(crate) group: String,
}

pub(crate) fn parse_link(link: &str) -> Result<PairingInvite, String> {
    PairingInvite::parse_link(link.trim()).map_err(|_| {
        tr!("Paste the whole pairing link from the other computer's Settings → Servers.")
            .to_string()
    })
}

fn pairing_failure(error: &ClientError) -> String {
    match error {
        ClientError::Server(reply) if reply.code == ErrorCode::Unauthorized => tr!(
            "This link has expired or was already used. Copy a new one from the other computer."
        )
        .to_string(),
        ClientError::Server(reply) => reply.message.clone(),
        ClientError::IdentityMismatch => tr!(
            "The computer at this link isn't the one that made it. Copy the link again from its Settings → Servers."
        )
        .to_string(),
        ClientError::Io(error) => tr!(
            "Can't reach the computer in this link (%@). Both computers must be on the same network or VPN. On macOS, also allow Muxy in System Settings → Privacy & Security → Local Network, then pair again.",
            error.to_string()
        )
        .to_string(),
        error => error.to_string(),
    }
}

fn computer_name() -> String {
    #[cfg(unix)]
    let name = rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .trim()
        .to_owned();
    #[cfg(windows)]
    let name = std::env::var("COMPUTERNAME").unwrap_or_default();
    let mut label = String::new();
    for character in name
        .strip_suffix(".local")
        .unwrap_or(&name)
        .chars()
        .filter(|character| !character.is_control())
    {
        if label.len() + character.len_utf8() > muxy_protocol::MAX_DEVICE_NAME {
            break;
        }
        label.push(character);
    }
    if label.is_empty() {
        tr!("Computer").to_string()
    } else {
        label
    }
}

impl AppModel {
    pub(crate) fn pairing_running(&self) -> bool {
        self.pairing.running.is_some()
    }

    pub(crate) fn start_pairing(
        &mut self,
        id: Option<ServerId>,
        form: PairingForm,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.pairing.running.is_some() {
            return Err(tr!("Wait for the current pairing to finish.").to_string());
        }
        let invite = parse_link(&form.link)?;
        self.check_pairing(id, &form, &invite.fingerprint)?;
        if let Some((spent, paired)) = self
            .pairing
            .unsaved
            .clone()
            .filter(|(spent, _)| *spent == invite)
        {
            let result = self.save_pairing(id, &form, &spent, &paired, cx);
            self.finish_pair_form(result, cx);
            return Ok(());
        }
        let paired = Self::pair_computer(invite.clone(), cx);
        self.pairing.running = Some(cx.spawn(async move |model, cx| {
            let paired = paired.await;
            let _ = model.update(cx, |model, cx| {
                model.pairing.running = None;
                let result =
                    paired.and_then(|paired| model.save_pairing(id, &form, &invite, &paired, cx));
                model.finish_pair_form(result, cx);
            });
        }));
        Ok(())
    }

    pub(super) fn save_pairing(
        &mut self,
        id: Option<ServerId>,
        form: &PairingForm,
        invite: &PairingInvite,
        paired: &Paired,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match self.save_paired_server(id, form, invite, paired, cx) {
            Ok(_) => {
                self.pairing.unsaved = None;
                Ok(())
            }
            Err(error) => {
                self.pairing.unsaved = Some((invite.clone(), paired.clone()));
                Err(tr!(
                    "The other computer accepted the pairing, but it couldn't be saved: %@. Fix that and pair again with the same link.",
                    error
                )
                .to_string())
            }
        }
    }

    fn check_pairing(
        &self,
        id: Option<ServerId>,
        form: &PairingForm,
        fingerprint: &[u8; 32],
    ) -> Result<(), String> {
        for (text, validate) in [
            (
                form.name.trim(),
                ServerEntry::validate_name as fn(&str) -> _,
            ),
            (form.group.trim(), ServerEntry::validate_group),
        ] {
            if !text.is_empty() {
                validate(text).map_err(|error| error.to_string())?;
            }
        }
        match (id, self.paired_with(fingerprint)) {
            (Some(id), Some(owner)) if owner.id != id => Err(tr!(
                "This link is for %@, which is already listed. Pair it again from its own Edit… form.",
                &owner.name
            )
            .to_string()),
            _ => Ok(()),
        }
    }

    fn paired_with(&self, fingerprint: &[u8; 32]) -> Option<&ServerEntry> {
        self.settings.servers.iter().find(|entry| {
            entry
                .paired
                .as_ref()
                .is_some_and(|paired| paired.has_certificate(fingerprint))
        })
    }

    fn pair_computer(
        invite: PairingInvite,
        cx: &mut Context<Self>,
    ) -> Task<Result<Paired, String>> {
        cx.background_executor().spawn(async move {
            let (client, paired) = Client::pair(&invite, &computer_name(), DeviceKind::Computer)
                .map_err(|error| pairing_failure(&error))?;
            client.disconnect();
            Ok(paired)
        })
    }

    pub(crate) fn save_paired_server(
        &mut self,
        id: Option<ServerId>,
        form: &PairingForm,
        invite: &PairingInvite,
        paired: &Paired,
        cx: &mut Context<Self>,
    ) -> Result<ServerId, String> {
        if self.quitting != Quitting::Idle {
            return Err(tr!("Muxy is quitting").to_string());
        }
        self.check_pairing(id, form, &invite.fingerprint)?;
        let endpoint = PairedServer {
            hosts: invite.hosts.clone(),
            port: invite.port,
            fingerprint: muxy_core::hex::encode(&invite.fingerprint),
            device: paired.credential.device,
        };
        let existing = id
            .and_then(|id| self.settings.server(id))
            .or_else(|| self.paired_with(&invite.fingerprint))
            .cloned();
        let name = match form.name.trim() {
            "" => existing
                .as_ref()
                .map_or_else(|| paired.name.clone(), |entry| entry.name.clone()),
            name => name.to_owned(),
        };
        let mut entry = ServerEntry::paired(name, endpoint);
        entry.group = match form.group.trim() {
            "" if id.is_none() => existing.as_ref().and_then(|entry| entry.group.clone()),
            "" => None,
            group => Some(group.to_owned()),
        };
        if let Some(existing) = &existing {
            entry.id = existing.id;
        }
        entry.validate().map_err(|error| error.to_string())?;
        let server = entry.id;
        let previous = PairingTokens::load(&self.servers.tokens)
            .map_err(|error| error.to_string())?
            .token(server);
        PairingTokens::set(&self.servers.tokens, server, &paired.credential.token)
            .map_err(|error| error.to_string())?;
        let path = self.path.with_file_name("settings.toml");
        let saved = if existing.is_some() {
            self.settings.update_server(entry, &path)
        } else {
            self.settings.add_server(entry, &path)
        };
        if let Err(error) = saved {
            let _ = match previous {
                Some(token) => PairingTokens::set(&self.servers.tokens, server, &token),
                None => PairingTokens::remove(&self.servers.tokens, &[server]),
            };
            return Err(error.to_string());
        }
        self.sync_servers(cx);
        self.connect_on_demand(server, cx);
        Ok(server)
    }

    pub(crate) fn edit_paired_server(
        &mut self,
        id: ServerId,
        form: &PairingForm,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let mut entry = self
            .settings
            .server(id)
            .filter(|entry| entry.paired.is_some())
            .cloned()
            .ok_or_else(|| tr!("This remote server is no longer listed").to_string())?;
        let name = form.name.trim();
        if !name.is_empty() {
            name.clone_into(&mut entry.name);
        }
        entry.group = Some(form.group.trim().to_owned()).filter(|group| !group.is_empty());
        let path = self.path.with_file_name("settings.toml");
        self.settings
            .update_server(entry, &path)
            .map_err(|error| error.to_string())?;
        self.sync_servers(cx);
        Ok(())
    }

    pub(super) fn name_unpaired(
        &self,
        server: ServerId,
        reason: Option<muxy_client::RemoteReason>,
    ) -> Option<String> {
        use muxy_client::RemoteReason::{IdentityChanged, Unpaired};
        let reason = reason.filter(|reason| matches!(reason, Unpaired | IdentityChanged))?;
        Some(
            ClientError::Remote {
                reason,
                destination: self.server_label(server),
                detail: String::new(),
            }
            .to_string(),
        )
    }

    pub(super) fn take_pairings(
        &self,
        servers: &[ServerId],
    ) -> Result<Vec<(ServerId, [u8; 32])>, String> {
        let paired: Vec<ServerId> = servers
            .iter()
            .copied()
            .filter(|server| {
                self.settings
                    .server(*server)
                    .is_some_and(|entry| entry.paired.is_some())
            })
            .collect();
        if paired.is_empty() {
            return Ok(Vec::new());
        }
        let failed = |error: muxy_app_core::settings::Error| {
            tr!("Could not remove the pairing: %@", error.to_string()).to_string()
        };
        let tokens = PairingTokens::load(&self.servers.tokens).map_err(failed)?;
        let taken = paired
            .iter()
            .filter_map(|server| tokens.token(*server).map(|token| (*server, token)))
            .collect();
        PairingTokens::remove(&self.servers.tokens, &paired).map_err(failed)?;
        Ok(taken)
    }

    pub(super) fn restore_pairings(&self, taken: &[(ServerId, [u8; 32])]) {
        for (server, token) in taken {
            let _ = PairingTokens::set(&self.servers.tokens, *server, token);
        }
    }
}

#[cfg(test)]
mod tests {
    use muxy_protocol::ErrorReply;

    use super::*;

    #[test]
    fn an_expired_link_says_to_copy_a_new_one() {
        let expired = ClientError::Server(ErrorReply {
            code: ErrorCode::Unauthorized,
            message: "this device is not paired with the server".into(),
        });
        assert!(pairing_failure(&expired).contains("expired"));
        let old = ClientError::Server(ErrorReply {
            code: ErrorCode::Unsupported,
            message: "Update it there.".into(),
        });
        assert_eq!(pairing_failure(&old), "Update it there.");
        let blocked = ClientError::Io(std::io::Error::from(std::io::ErrorKind::HostUnreachable));
        assert!(pairing_failure(&blocked).contains("Local Network"));
    }
}
