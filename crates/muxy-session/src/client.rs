use crate::transport::{
    client_handshake, read_frame, read_structured, write_frame, write_structured,
};
use muxy_proto::session::codec::{FrameKind, decode_output};
use muxy_proto::session::{
    AttachRequest, Attached, BuildMode, ClientKind, ControlRequest, ControlResponse,
    CreateSessionRequest, Resize, SessionDescriptor, SessionExited, SessionId,
};
use std::io;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);

pub struct SessionClient {
    stream: UnixStream,
    next_request_id: u64,
    daemon: muxy_proto::session::ProcessIdentity,
}

impl SessionClient {
    pub fn connect(socket_path: impl AsRef<Path>, build_mode: BuildMode) -> io::Result<Self> {
        let mut stream = UnixStream::connect(socket_path)?;
        set_connection_timeout(&stream, Some(CONNECTION_TIMEOUT))?;
        let accepted = client_handshake(&mut stream, ClientKind::Control, build_mode)?;
        set_connection_timeout(&stream, None)?;
        Ok(Self {
            stream,
            next_request_id: 1,
            daemon: accepted.daemon,
        })
    }

    pub fn connect_or_spawn(
        socket_path: impl AsRef<Path>,
        helper_path: impl AsRef<Path>,
        build_mode: BuildMode,
    ) -> io::Result<Self> {
        let socket_path = socket_path.as_ref();
        match Self::connect(socket_path, build_mode) {
            Ok(client) => return Ok(client),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound
                        | io::ErrorKind::ConnectionRefused
                        | io::ErrorKind::ConnectionReset
                ) => {}
            Err(error) => return Err(error),
        }
        spawn_daemon(helper_path.as_ref(), socket_path)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match Self::connect(socket_path, build_mode) {
                Ok(client) => return Ok(client),
                Err(error) if Instant::now() < deadline => {
                    if !matches!(
                        error.kind(),
                        io::ErrorKind::NotFound
                            | io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::ConnectionReset
                    ) {
                        return Err(error);
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub fn daemon_identity(&self) -> muxy_proto::session::ProcessIdentity {
        self.daemon
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_write_timeout(timeout)
    }

    pub fn list_sessions(&mut self) -> io::Result<Vec<SessionDescriptor>> {
        match self.request(ControlRequest::ListSessions)? {
            ControlResponse::Sessions(sessions) => Ok(sessions),
            response => Err(unexpected(response)),
        }
    }

    pub fn get_session(&mut self, session_id: SessionId) -> io::Result<Option<SessionDescriptor>> {
        match self.request(ControlRequest::GetSession { session_id })? {
            ControlResponse::Session(session) => Ok(session),
            response => Err(unexpected(response)),
        }
    }

    pub fn create_session(
        &mut self,
        request: CreateSessionRequest,
    ) -> io::Result<SessionDescriptor> {
        match self.request(ControlRequest::CreateSession(Box::new(request)))? {
            ControlResponse::Created(session) => Ok(session),
            ControlResponse::DuplicateOwnerConflict => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "session owner launch contract differs",
            )),
            response => Err(unexpected(response)),
        }
    }

    pub fn end_session(&mut self, session_id: SessionId) -> io::Result<()> {
        self.expect_acknowledged(ControlRequest::EndSession { session_id })
    }

    pub fn acknowledge_exited_session(&mut self, session_id: SessionId) -> io::Result<()> {
        self.expect_acknowledged(ControlRequest::AcknowledgeExitedSession { session_id })
    }

    pub fn end_sessions_by_owner(
        &mut self,
        owner: muxy_proto::session::SessionOwner,
    ) -> io::Result<()> {
        self.expect_acknowledged(ControlRequest::EndSessionsByOwner { owner })
    }

    pub fn end_all_sessions(&mut self) -> io::Result<()> {
        self.expect_acknowledged(ControlRequest::EndAllSessions)
    }

    pub fn set_workspace_placement(
        &mut self,
        session_id: SessionId,
        placement: Option<muxy_proto::session::WorkspacePlacement>,
    ) -> io::Result<()> {
        self.expect_acknowledged(ControlRequest::SetWorkspacePlacement {
            session_id,
            placement,
        })
    }

    pub fn ping(&mut self) -> io::Result<()> {
        match self.request(ControlRequest::Ping)? {
            ControlResponse::Pong => Ok(()),
            response => Err(unexpected(response)),
        }
    }

    fn expect_acknowledged(&mut self, request: ControlRequest) -> io::Result<()> {
        match self.request(request)? {
            ControlResponse::Acknowledged => Ok(()),
            response => Err(unexpected(response)),
        }
    }

    fn request(&mut self, request: ControlRequest) -> io::Result<ControlResponse> {
        let request_id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("session request IDs exhausted"))?;
        write_structured(
            &mut self.stream,
            FrameKind::ControlRequest,
            request_id,
            &request,
        )?;
        let response = read_frame(&mut self.stream)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "daemon closed"))?;
        if response.header.kind != FrameKind::ControlResponse
            || response.header.request_id != request_id
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "session response correlation differs",
            ));
        }
        let response: ControlResponse = read_structured(&response)?;
        match response {
            ControlResponse::Error { code, message } => Err(io::Error::other(format!(
                "session daemon {code}: {message}"
            ))),
            response => Ok(response),
        }
    }
}

pub struct RendererClient {
    stream: UnixStream,
    pub attached: Attached,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RendererEvent {
    Output { sequence: u64, bytes: Vec<u8> },
    Exited(SessionExited),
}

impl RendererClient {
    pub fn connect(
        socket_path: impl AsRef<Path>,
        build_mode: BuildMode,
        request: AttachRequest,
    ) -> io::Result<Self> {
        let mut stream = UnixStream::connect(socket_path)?;
        set_connection_timeout(&stream, Some(CONNECTION_TIMEOUT))?;
        client_handshake(&mut stream, ClientKind::Renderer, build_mode)?;
        write_structured(&mut stream, FrameKind::Attach, 0, &request)?;
        let response = read_frame(&mut stream)?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "daemon closed before attach")
        })?;
        if response.header.kind != FrameKind::Attached {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "daemon did not accept renderer attachment",
            ));
        }
        let attached: Attached = read_structured(&response)?;
        if attached.session.session_id != request.session_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "attached identity differs",
            ));
        }
        set_connection_timeout(&stream, None)?;
        Ok(Self { stream, attached })
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_write_timeout(timeout)
    }

    pub fn send_input(&mut self, bytes: &[u8]) -> io::Result<()> {
        write_frame(&mut self.stream, FrameKind::Input, 0, bytes)
    }

    pub fn send_resize(&mut self, resize: Resize) -> io::Result<()> {
        write_structured(&mut self.stream, FrameKind::Resize, 0, &resize)
    }

    pub fn next_event(&mut self) -> io::Result<Option<RendererEvent>> {
        let Some(frame) = read_frame(&mut self.stream)? else {
            return Ok(None);
        };
        match frame.header.kind {
            FrameKind::Output => {
                let (sequence, bytes) = decode_output(&frame.payload).map_err(|error| {
                    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
                })?;
                Ok(Some(RendererEvent::Output {
                    sequence,
                    bytes: bytes.to_vec(),
                }))
            }
            FrameKind::Exited => Ok(Some(RendererEvent::Exited(read_structured(&frame)?))),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected renderer event",
            )),
        }
    }

    pub fn try_clone_stream(&self) -> io::Result<UnixStream> {
        self.stream.try_clone()
    }

    pub fn disconnect(self) -> io::Result<()> {
        self.stream.shutdown(Shutdown::Both)
    }
}

pub fn sibling_helper(current_executable: impl AsRef<Path>) -> io::Result<PathBuf> {
    let directory = current_executable.as_ref().parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "app executable has no parent")
    })?;
    let helper = directory.join("muxy-session");
    validate_helper(&helper)?;
    Ok(helper)
}

fn spawn_daemon(helper_path: &Path, socket_path: &Path) -> io::Result<()> {
    validate_helper(helper_path)?;
    let mut command = Command::new(helper_path);
    command
        .arg("daemon")
        .arg("--socket")
        .arg(socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        std::os::unix::process::CommandExt::pre_exec(&mut command, || {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn()?;
    Ok(())
}

fn set_connection_timeout(stream: &UnixStream, timeout: Option<Duration>) -> io::Result<()> {
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)
}

fn validate_helper(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.file_name().and_then(|name| name.to_str()) != Some("muxy-session")
        || !path.is_file()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "session helper path is invalid",
        ));
    }
    Ok(())
}

fn unexpected(response: ControlResponse) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("unexpected session response: {response:?}"),
    )
}
