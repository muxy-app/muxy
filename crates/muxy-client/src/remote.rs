//! Connections to a server over the network, as a paired device.

use std::io;
use std::time::Duration;

use muxy_protocol::transport::{ByteStream, tls};
use muxy_protocol::{
    DeviceCredential, DeviceId, DeviceKind, ErrorCode, ErrorReply, Feature, PairRequest, Paired,
    PairingInvite, PairingOffer, RemoteAccessSettings, RemoteAccessState, ReplyBody, RequestBody,
};

use crate::{Client, ClientError};

/// Budget for reaching and verifying one address of a server.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);

/// Where a paired server can be reached, and the certificate it must present.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteEndpoint {
    pub hosts: Vec<String>,
    pub port: u16,
    pub fingerprint: [u8; 32],
}

impl From<&PairingInvite> for RemoteEndpoint {
    fn from(invite: &PairingInvite) -> Self {
        Self {
            hosts: invite.hosts.clone(),
            port: invite.port,
            fingerprint: invite.fingerprint,
        }
    }
}

impl Client {
    /// Connects as a paired device, trying each host in order.
    pub fn connect_remote(
        endpoint: &RemoteEndpoint,
        credential: DeviceCredential,
    ) -> Result<Self, ClientError> {
        let client = Self::from_stream_with_timeout(connect(endpoint)?, CONNECT_TIMEOUT)?;
        match client.request(RequestBody::Authenticate(credential))? {
            ReplyBody::Authenticated => Ok(client),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    /// Pairs with the server that issued the invite; the connection stays open.
    /// A server too old to pair computers is refused before it could pair one
    /// as a phone.
    pub fn pair(
        invite: &PairingInvite,
        name: &str,
        kind: DeviceKind,
    ) -> Result<(Self, Paired), ClientError> {
        let stream = connect(&RemoteEndpoint::from(invite))?;
        let client = Self::from_stream_with_timeout(stream, CONNECT_TIMEOUT)?;
        let paired = client.request_pairing(invite, name, kind);
        if paired.is_err() {
            client.disconnect();
        }
        Ok((client, paired?))
    }

    fn request_pairing(
        &self,
        invite: &PairingInvite,
        name: &str,
        kind: DeviceKind,
    ) -> Result<Paired, ClientError> {
        if kind == DeviceKind::Computer && !self.supports(Feature::PAIRED_COMPUTERS) {
            return Err(ClientError::Server(ErrorReply {
                code: ErrorCode::Unsupported,
                message: "Muxy on that computer can't pair with other computers. Update it there."
                    .into(),
            }));
        }
        match self.request(RequestBody::Pair(PairRequest {
            secret: invite.secret,
            name: name.into(),
            kind,
        }))? {
            ReplyBody::Paired(paired) => Ok(paired),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    pub fn read_remote_access(&self) -> Result<RemoteAccessState, ClientError> {
        remote_access(self.request(RequestBody::ReadRemoteAccess)?)
    }

    pub fn write_remote_access(
        &self,
        settings: RemoteAccessSettings,
    ) -> Result<RemoteAccessState, ClientError> {
        remote_access(self.request(RequestBody::WriteRemoteAccess(settings))?)
    }

    pub fn start_pairing(&self) -> Result<PairingOffer, ClientError> {
        match self.request(RequestBody::StartPairing)? {
            ReplyBody::Pairing(offer) => Ok(offer),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    /// Like [`Client::start_pairing`], but the link lists `hosts` before the
    /// addresses the server finds itself, for a server that phones reach by
    /// another name, such as behind NAT. Without hosts it sends the plain
    /// request, which older servers also know.
    pub fn start_pairing_with(&self, hosts: Vec<String>) -> Result<PairingOffer, ClientError> {
        if hosts.is_empty() {
            return self.start_pairing();
        }
        muxy_protocol::validate_pairing_hosts(&hosts).map_err(ClientError::Invalid)?;
        match self.request(RequestBody::StartPairingWithHosts(hosts)) {
            Ok(ReplyBody::Pairing(offer)) => Ok(offer),
            Ok(body) => Err(ClientError::UnexpectedReply(Box::new(body))),
            Err(ClientError::Server(error)) if error.code == ErrorCode::Unsupported => {
                Err(ClientError::Server(ErrorReply {
                    code: ErrorCode::Unsupported,
                    message:
                        "This server can't add addresses to a pairing code. Update Muxy on it."
                            .into(),
                }))
            }
            Err(error) => Err(error),
        }
    }

    pub fn cancel_pairing(&self) -> Result<RemoteAccessState, ClientError> {
        remote_access(self.request(RequestBody::CancelPairing)?)
    }

    pub fn revoke_device(&self, device: DeviceId) -> Result<RemoteAccessState, ClientError> {
        remote_access(self.request(RequestBody::RevokeDevice(device))?)
    }
}

fn remote_access(body: ReplyBody) -> Result<RemoteAccessState, ClientError> {
    match body {
        ReplyBody::RemoteAccess(state) => Ok(state),
        body => Err(ClientError::UnexpectedReply(Box::new(body))),
    }
}

/// Whether the paired server still takes connections: `Some(false)` once a
/// host refuses them, `None` when no host answers either way, such as off the
/// network.
pub fn accepts_connections(endpoint: &RemoteEndpoint) -> Option<bool> {
    let mut refused = false;
    for host in &endpoint.hosts {
        match tls::connect(host, endpoint.port, endpoint.fingerprint, CONNECT_TIMEOUT) {
            Ok(_) => return Some(true),
            Err(error) => refused |= error.kind() == io::ErrorKind::ConnectionRefused,
        }
    }
    refused.then_some(false)
}

/// A host presenting another certificate is skipped, never trusted; the
/// mismatch is reported only if no host answers with the pinned one.
fn connect(endpoint: &RemoteEndpoint) -> Result<Box<dyn ByteStream>, ClientError> {
    let mut mismatch = false;
    let mut failure = None;
    for host in &endpoint.hosts {
        match tls::connect(host, endpoint.port, endpoint.fingerprint, CONNECT_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                mismatch |= tls::is_identity_mismatch(&error);
                failure = Some(error);
            }
        }
    }
    if mismatch {
        return Err(ClientError::IdentityMismatch);
    }
    Err(ClientError::Io(failure.unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "the server has no known address")
    })))
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;
    use std::thread;

    use muxy_protocol::wire::{Decoder, Encoder};
    use muxy_protocol::{CONTROL, Message, SUPPORTED, ServerIdentity, ServerInfo};

    use super::*;

    fn invite() -> PairingInvite {
        PairingInvite {
            hosts: vec!["studio.local".into()],
            port: 7419,
            fingerprint: [1; 32],
            secret: [2; 16],
        }
    }

    /// A server offering `features` that pairs whoever asks, and reports the
    /// pairing request it received.
    fn server(
        features: Vec<Feature>,
    ) -> Result<(Client, mpsc::Receiver<PairRequest>), ClientError> {
        let (near, far) = UnixStream::pair()?;
        let (requests, received) = mpsc::channel();
        thread::spawn(move || -> Result<(), muxy_protocol::wire::WireError> {
            let (read, write) = (far.try_clone()?, far);
            let (mut decoder, mut encoder) = (Decoder::new(read), Encoder::new(write));
            decoder.next()?;
            encoder.send(
                CONTROL,
                &Message::HelloReply {
                    versions: SUPPORTED.to_vec(),
                    server: ServerInfo::current(),
                    features,
                },
            )?;
            if let (
                _,
                Message::Request {
                    id,
                    body: RequestBody::Pair(request),
                },
            ) = decoder.next()?
            {
                let _ = requests.send(request);
                let paired = Paired {
                    credential: DeviceCredential {
                        device: DeviceId::from_u128(3),
                        token: [4; 32],
                    },
                    server: ServerIdentity::from_u128(5),
                    name: "Studio".into(),
                };
                encoder.send(
                    CONTROL,
                    &Message::Reply {
                        id,
                        body: ReplyBody::Paired(paired),
                    },
                )?;
            }
            Ok(())
        });
        Ok((Client::from_stream(Box::new(near))?, received))
    }

    #[test]
    fn a_stopped_server_refuses_connections() {
        // Nothing listens on port 1 here, and no other test can take it.
        let stopped = RemoteEndpoint {
            hosts: vec!["127.0.0.1".into()],
            port: 1,
            fingerprint: [1; 32],
        };
        assert_eq!(accepts_connections(&stopped), Some(false));
        let nowhere = RemoteEndpoint {
            hosts: Vec::new(),
            ..stopped
        };
        assert_eq!(accepts_connections(&nowhere), None, "unknown, not stopped");
    }

    #[test]
    fn a_server_that_cannot_pair_computers_is_never_asked() -> Result<(), ClientError> {
        let (client, requests) = server(Vec::new())?;
        let refused = client.request_pairing(&invite(), "Laptop", DeviceKind::Computer);
        assert!(
            matches!(&refused, Err(ClientError::Server(error)) if error.code == ErrorCode::Unsupported),
            "{refused:?}"
        );
        client.disconnect();
        assert!(requests.recv().is_err());
        Ok(())
    }

    #[test]
    fn a_computer_pairs_as_a_computer() -> Result<(), ClientError> {
        let (client, requests) = server(vec![Feature::PAIRED_COMPUTERS])?;
        let paired = client.request_pairing(&invite(), "Laptop", DeviceKind::Computer)?;
        assert_eq!(paired.name, "Studio");
        let request = requests.recv().map_err(|_| ClientError::Disconnected)?;
        assert_eq!(
            (request.kind, request.secret),
            (DeviceKind::Computer, [2; 16])
        );
        Ok(())
    }
}
