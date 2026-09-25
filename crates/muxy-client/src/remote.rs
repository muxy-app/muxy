//! Connections to a server over the network, as a paired device.

use std::io;
use std::time::Duration;

use muxy_protocol::transport::{ByteStream, tls};
use muxy_protocol::{
    DeviceCredential, DeviceId, PairRequest, Paired, PairingInvite, PairingOffer,
    RemoteAccessSettings, RemoteAccessState, ReplyBody, RequestBody,
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
    pub fn pair(invite: &PairingInvite, name: &str) -> Result<(Self, Paired), ClientError> {
        let stream = connect(&RemoteEndpoint::from(invite))?;
        let client = Self::from_stream_with_timeout(stream, CONNECT_TIMEOUT)?;
        match client.request(RequestBody::Pair(PairRequest {
            secret: invite.secret,
            name: name.into(),
        }))? {
            ReplyBody::Paired(paired) => Ok((client, paired)),
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
