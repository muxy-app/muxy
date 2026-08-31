use muxy_proto::session::codec::{
    FrameHeader, FrameKind, HEADER_BYTES, decode_header, decode_structured, encode_frame,
    encode_structured,
};
use muxy_proto::session::{
    BuildMode, ClientKind, Hello, HelloAccepted, ProcessIdentity, ProtocolVersion, VersionMismatch,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::{self, Read, Write};
use std::mem::{size_of, zeroed};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerIdentity {
    pub process_id: u32,
    pub user_id: u32,
}

#[derive(Debug)]
pub struct WireFrame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

pub fn authenticate_same_user(stream: &UnixStream) -> io::Result<PeerIdentity> {
    let peer = peer_identity(stream)?;
    validate_peer(peer, effective_uid())?;
    Ok(peer)
}

pub fn validate_peer(peer: PeerIdentity, expected_user_id: u32) -> io::Result<()> {
    if peer.user_id != expected_user_id || peer.process_id == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "session peer identity differs",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn peer_identity(stream: &UnixStream) -> io::Result<PeerIdentity> {
    let fd = stream.as_raw_fd();
    let mut credentials: libc::xucred = unsafe { zeroed() };
    let mut credential_length = size_of::<libc::xucred>() as libc::socklen_t;
    let credential_result = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_LOCAL,
            libc::LOCAL_PEERCRED,
            (&raw mut credentials).cast(),
            &raw mut credential_length,
        )
    };
    if credential_result != 0 || credential_length as usize != size_of::<libc::xucred>() {
        return Err(io::Error::last_os_error());
    }
    let mut process_id: libc::pid_t = 0;
    let mut process_length = size_of::<libc::pid_t>() as libc::socklen_t;
    let process_result = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&raw mut process_id).cast(),
            &raw mut process_length,
        )
    };
    if process_result != 0 || process_length as usize != size_of::<libc::pid_t>() {
        return Err(io::Error::last_os_error());
    }
    Ok(PeerIdentity {
        process_id: u32::try_from(process_id)
            .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "invalid peer PID"))?,
        user_id: credentials.cr_uid,
    })
}

#[cfg(target_os = "linux")]
fn peer_identity(stream: &UnixStream) -> io::Result<PeerIdentity> {
    let mut credentials: libc::ucred = unsafe { zeroed() };
    let mut length = size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut credentials).cast(),
            &raw mut length,
        )
    };
    if result != 0 || length as usize != size_of::<libc::ucred>() {
        return Err(io::Error::last_os_error());
    }
    Ok(PeerIdentity {
        process_id: u32::try_from(credentials.pid)
            .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "invalid peer PID"))?,
        user_id: credentials.uid,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn peer_identity(_stream: &UnixStream) -> io::Result<PeerIdentity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "session peer credentials are unsupported",
    ))
}

pub fn read_frame(stream: &mut UnixStream) -> io::Result<Option<WireFrame>> {
    let mut header_bytes = [0u8; HEADER_BYTES];
    let mut first = [0u8; 1];
    match stream.read(&mut first) {
        Ok(0) => return Ok(None),
        Ok(1) => header_bytes[0] = first[0],
        Ok(_) => unreachable!(),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => return read_frame(stream),
        Err(error) => return Err(error),
    }
    stream.read_exact(&mut header_bytes[1..])?;
    let header = decode_header(&header_bytes).map_err(protocol_error)?;
    let mut payload = vec![0; header.payload_len as usize];
    stream.read_exact(&mut payload)?;
    Ok(Some(WireFrame { header, payload }))
}

pub fn write_frame(
    stream: &mut UnixStream,
    kind: FrameKind,
    request_id: u64,
    payload: &[u8],
) -> io::Result<()> {
    let bytes = encode_frame(kind, request_id, payload).map_err(protocol_error)?;
    stream.write_all(&bytes)
}

pub fn read_structured<T: DeserializeOwned>(frame: &WireFrame) -> io::Result<T> {
    decode_structured(&frame.payload).map_err(protocol_error)
}

pub fn write_structured<T: Serialize>(
    stream: &mut UnixStream,
    kind: FrameKind,
    request_id: u64,
    value: &T,
) -> io::Result<()> {
    let payload = encode_structured(value).map_err(protocol_error)?;
    write_frame(stream, kind, request_id, &payload)
}

pub fn client_handshake(
    stream: &mut UnixStream,
    client_kind: ClientKind,
    expected_mode: BuildMode,
) -> io::Result<HelloAccepted> {
    let hello = Hello {
        version: ProtocolVersion::CURRENT,
        client_kind,
        process_id: std::process::id(),
        nonce: random_nonce()?,
    };
    write_structured(stream, FrameKind::Hello, 0, &hello)?;
    let response = read_frame(stream)?.ok_or_else(|| eof("daemon closed during hello"))?;
    match response.header.kind {
        FrameKind::HelloAccepted => {
            let accepted: HelloAccepted = read_structured(&response)?;
            if accepted.build_mode != expected_mode {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "session daemon build mode differs",
                ));
            }
            ProtocolVersion::CURRENT
                .negotiate(accepted.version)
                .map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("session protocol mismatch: {error:?}"),
                    )
                })?;
            Ok(accepted)
        }
        FrameKind::VersionMismatch => {
            let mismatch: VersionMismatch = read_structured(&response)?;
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "session protocol mismatch: daemon {}.{}, client {}.{}",
                    mismatch.supported.major,
                    mismatch.supported.minor,
                    mismatch.received.major,
                    mismatch.received.minor
                ),
            ))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected daemon hello response",
        )),
    }
}

pub fn server_handshake(
    stream: &mut UnixStream,
    peer: PeerIdentity,
    daemon: ProcessIdentity,
    build_mode: BuildMode,
    daemon_nonce: [u8; 32],
) -> io::Result<ClientKind> {
    let frame = read_frame(stream)?.ok_or_else(|| eof("client closed before hello"))?;
    if frame.header.kind != FrameKind::Hello {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "first session frame must be hello",
        ));
    }
    let hello: Hello = read_structured(&frame)?;
    if hello.process_id != peer.process_id {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "hello PID differs from authenticated peer",
        ));
    }
    let version = match ProtocolVersion::CURRENT.negotiate(hello.version) {
        Ok(version) => version,
        Err(mismatch) => {
            write_structured(stream, FrameKind::VersionMismatch, 0, &mismatch)?;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "session protocol major version differs",
            ));
        }
    };
    let accepted = HelloAccepted {
        version,
        daemon,
        daemon_nonce,
        build_mode,
    };
    write_structured(stream, FrameKind::HelloAccepted, 0, &accepted)?;
    Ok(hello.client_kind)
}

pub fn random_nonce() -> io::Result<[u8; 32]> {
    let mut nonce = [0u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    Ok(nonce)
}

fn effective_uid() -> u32 {
    unsafe { libc::geteuid() }
}

fn protocol_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn eof(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_validation_rejects_foreign_uid_and_zero_pid() {
        let uid = effective_uid();
        assert!(
            validate_peer(
                PeerIdentity {
                    process_id: 1,
                    user_id: uid
                },
                uid
            )
            .is_ok()
        );
        assert!(
            validate_peer(
                PeerIdentity {
                    process_id: 1,
                    user_id: uid.wrapping_add(1)
                },
                uid
            )
            .is_err()
        );
        assert!(
            validate_peer(
                PeerIdentity {
                    process_id: 0,
                    user_id: uid
                },
                uid
            )
            .is_err()
        );
    }

    #[test]
    fn frame_reader_rejects_oversized_header_before_payload() {
        let (mut sender, mut receiver) = UnixStream::pair().unwrap();
        let header = FrameHeader {
            version: ProtocolVersion::CURRENT,
            kind: FrameKind::Input,
            flags: 0,
            request_id: 0,
            payload_len: (muxy_proto::session::MAX_STREAM_CHUNK_BYTES + 1) as u32,
        };
        sender.write_all(&header.encode()).unwrap();
        assert_eq!(
            read_frame(&mut receiver).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
