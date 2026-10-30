//! Pairing with another computer from the link its Settings → Devices shows.
//! The server's entry lives in `settings.toml` and its token for this
//! computer in `paired-servers.toml`.

use gpui::{Context, Task};
use muxy_app_core::ServerId;
use muxy_app_core::settings::{PairedServer, PairingTokens, ServerEntry};
use muxy_client::{Client, ClientError};
use muxy_protocol::{DeviceKind, ErrorCode, Paired, PairingInvite};
use muxy_ui::tr;

use super::{AppModel, Quitting};

/// What the pairing form submits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PairingForm {
    pub(crate) link: String,
    /// Empty keeps the name it had, or the one the other computer reports.
    pub(crate) name: String,
    /// Empty leaves it ungrouped.
    pub(crate) group: String,
}

pub(crate) fn parse_link(link: &str) -> Result<PairingInvite, String> {
    PairingInvite::parse_link(link.trim()).map_err(|_| {
        tr!("Paste the whole pairing link from the other computer's Settings → Devices.")
            .to_string()
    })
}

/// Why pairing failed, in the words of what to do about it.
fn pairing_failure(error: &ClientError) -> String {
    match error {
        ClientError::Server(reply) if reply.code == ErrorCode::Unauthorized => tr!(
            "This link has expired or was already used. Copy a new one from the other computer."
        )
        .to_string(),
        ClientError::Server(reply) => reply.message.clone(),
        ClientError::IdentityMismatch => tr!(
            "The computer at this link isn't the one that made it. Copy the link again from its Settings → Devices."
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

/// This computer as the other one lists it among its paired devices.
fn computer_name() -> String {
    let name = rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .trim()
        .to_owned();
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
    /// Pairs with the computer that showed `invite`, off the UI thread.
    pub(crate) fn pair_computer(
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

    /// Lists a computer this one just paired with, and connects to it. One
    /// already listed, by `id` or by the same certificate, is updated in place,
    /// so pairing again never lists it twice. Pairing again from its own form
    /// (`id`) takes the group as typed. The token is saved first: if the entry
    /// can't be, the token it had before is put back.
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
        let endpoint = PairedServer {
            hosts: invite.hosts.clone(),
            port: invite.port,
            fingerprint: hex(&invite.fingerprint),
            device: paired.credential.device,
        };
        let existing = id
            .and_then(|id| self.settings.server(id))
            .or_else(|| {
                self.settings.servers.iter().find(|entry| {
                    entry
                        .paired
                        .as_ref()
                        .is_some_and(|known| known.fingerprint == endpoint.fingerprint)
                })
            })
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
        Ok(server)
    }

    /// Renames or regroups a paired server without pairing again.
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

    /// Why a paired server stopped accepting this computer, by the name the
    /// user gave it, which its worker doesn't know.
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

    /// Drops the tokens of servers no longer listed. Their pairing stays on
    /// the other computer until it is revoked there.
    pub(super) fn forget_pairings(&mut self, servers: &[ServerId], cx: &mut Context<Self>) {
        if let Err(error) = PairingTokens::remove(&self.servers.tokens, servers) {
            self.show_notice(
                tr!("Could not remove the pairing: %@", error.to_string()).to_string(),
                cx,
            );
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
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
