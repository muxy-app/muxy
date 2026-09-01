use muxy_proto::session::{
    Empty, HEADER_BYTES, MAX_FRAME_PAYLOAD_BYTES, QueryResult, SessionCodec, SessionDescriptor,
    SessionMessage, SessionQuery, TerminationOutcome,
};
use std::io::{Read, Write};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const CONTROL_TIMEOUT: Duration = Duration::from_millis(750);
const TERMINATION_TIMEOUT: Duration = Duration::from_secs(10);
const RETRY_INTERVAL: Duration = Duration::from_millis(100);
const RETRY_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub struct SessionClient {
    socket_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryOutcome {
    Available(Vec<SessionDescriptor>),
    Unreachable(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryOutcome {
    Found(SessionDescriptor),
    Missing,
    Unreachable(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminateOutcome {
    Terminated,
    NoSessions,
    Unreachable(String),
}

impl SessionClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    pub fn recover(&self) -> InventoryOutcome {
        if !self.socket_exists() {
            return InventoryOutcome::Available(Vec::new());
        }
        match self.exchange(SessionMessage::Recover(Empty {}), CONTROL_TIMEOUT) {
            Ok(SessionMessage::Recovered(descriptors)) => InventoryOutcome::Available(descriptors),
            Ok(message) => InventoryOutcome::Unreachable(unexpected(&message)),
            Err(error) => InventoryOutcome::Unreachable(error),
        }
    }

    pub fn query(&self, session_id: &str) -> QueryOutcome {
        if !self.socket_exists() {
            return QueryOutcome::Missing;
        }
        let request = SessionMessage::Query(SessionQuery {
            session_id: session_id.to_owned(),
        });
        match self.exchange(request, CONTROL_TIMEOUT) {
            Ok(SessionMessage::QueryResult(QueryResult::Found(descriptor))) => {
                QueryOutcome::Found(descriptor)
            }
            Ok(SessionMessage::QueryResult(QueryResult::Missing)) => QueryOutcome::Missing,
            Ok(message) => QueryOutcome::Unreachable(unexpected(&message)),
            Err(error) => QueryOutcome::Unreachable(error),
        }
    }

    pub fn wait_for_session(&self, session_id: &str) -> QueryOutcome {
        let deadline = Instant::now() + RETRY_TIMEOUT;
        loop {
            let outcome = self.query(session_id);
            if matches!(outcome, QueryOutcome::Found(_)) || Instant::now() >= deadline {
                return outcome;
            }
            std::thread::sleep(RETRY_INTERVAL);
        }
    }

    pub fn terminate_one(&self, session_id: &str) -> TerminateOutcome {
        if !self.socket_exists() {
            return TerminateOutcome::NoSessions;
        }
        let request = SessionMessage::TerminateOne(SessionQuery {
            session_id: session_id.to_owned(),
        });
        self.termination_exchange(request)
    }

    pub fn terminate_all(&self) -> TerminateOutcome {
        if !self.socket_exists() {
            return TerminateOutcome::NoSessions;
        }
        self.termination_exchange(SessionMessage::TerminateAll(Empty {}))
    }

    fn termination_exchange(&self, request: SessionMessage) -> TerminateOutcome {
        match self.exchange(request, TERMINATION_TIMEOUT) {
            Ok(SessionMessage::TerminateResult(result)) => match result.outcome {
                TerminationOutcome::Terminated => TerminateOutcome::Terminated,
                TerminationOutcome::NoSessions => TerminateOutcome::NoSessions,
            },
            Ok(message) => TerminateOutcome::Unreachable(unexpected(&message)),
            Err(error) => TerminateOutcome::Unreachable(error),
        }
    }

    fn socket_exists(&self) -> bool {
        match std::fs::symlink_metadata(&self.socket_path) {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => true,
        }
    }

    fn exchange(
        &self,
        request: SessionMessage,
        timeout: Duration,
    ) -> Result<SessionMessage, String> {
        let metadata = std::fs::symlink_metadata(&self.socket_path)
            .map_err(|error| format!("session socket metadata failed: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_socket() {
            return Err("session socket is not a real Unix socket".to_owned());
        }
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|error| format!("session daemon connection failed: {error}"))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| format!("session daemon read timeout failed: {error}"))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|error| format!("session daemon write timeout failed: {error}"))?;
        let frame = SessionCodec::encode(&request)
            .map_err(|error| format!("session request encoding failed: {error}"))?;
        stream
            .write_all(&frame)
            .map_err(|error| format!("session request failed: {error}"))?;
        read_message(&mut stream)
    }
}

fn read_message(stream: &mut UnixStream) -> Result<SessionMessage, String> {
    let mut header = [0u8; HEADER_BYTES];
    stream
        .read_exact(&mut header)
        .map_err(|error| format!("session response header failed: {error}"))?;
    let payload_len = u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;
    if payload_len > MAX_FRAME_PAYLOAD_BYTES {
        return Err(format!(
            "session response exceeds {MAX_FRAME_PAYLOAD_BYTES} bytes"
        ));
    }
    let mut frame = Vec::with_capacity(HEADER_BYTES + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(HEADER_BYTES + payload_len, 0);
    stream
        .read_exact(&mut frame[HEADER_BYTES..])
        .map_err(|error| format!("session response payload failed: {error}"))?;
    SessionCodec::decode(&frame).map_err(|error| format!("session response is invalid: {error}"))
}

fn unexpected(message: &SessionMessage) -> String {
    match message {
        SessionMessage::ProtocolError(error) => {
            format!("session daemon rejected the request: {}", error.message)
        }
        _ => format!("unexpected session response: {:?}", message.kind()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::sync::Arc;

    fn serve(response: SessionMessage) -> (tempfile::TempDir, SessionClient) {
        let root = tempfile::TempDir::new().unwrap();
        let socket = root.path().join("control.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let response = Arc::new(response);
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_message(&mut stream).unwrap();
            stream
                .write_all(&SessionCodec::encode(response.as_ref()).unwrap())
                .unwrap();
        });
        (root, SessionClient::new(socket))
    }

    #[test]
    fn absent_socket_has_distinct_safe_outcomes() {
        let root = tempfile::TempDir::new().unwrap();
        let client = SessionClient::new(root.path().join("missing.sock"));
        assert_eq!(client.recover(), InventoryOutcome::Available(Vec::new()));
        assert_eq!(
            client.query("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"),
            QueryOutcome::Missing
        );
        assert_eq!(client.terminate_all(), TerminateOutcome::NoSessions);
    }

    #[test]
    fn query_preserves_found_missing_and_unreachable() {
        let descriptor = SessionDescriptor {
            session_id: "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE".to_owned(),
            owner: muxy_proto::session::OwnerMetadata {
                project_id: "11111111-2222-4333-8444-555555555555".to_owned(),
                worktree_id: None,
                title: "Shell".to_owned(),
            },
            working_directory: "/tmp".to_owned(),
            shell_pid: 1,
            tty_device: 1,
            command_activity: muxy_proto::session::CommandActivity::Idle,
        };
        let (_root, client) = serve(SessionMessage::QueryResult(QueryResult::Found(
            descriptor.clone(),
        )));
        assert_eq!(
            client.query(&descriptor.session_id),
            QueryOutcome::Found(descriptor)
        );

        let (_root, client) = serve(SessionMessage::QueryResult(QueryResult::Missing));
        assert_eq!(
            client.query("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"),
            QueryOutcome::Missing
        );

        let root = tempfile::TempDir::new().unwrap();
        std::fs::write(root.path().join("control.sock"), b"not a socket").unwrap();
        assert!(matches!(
            SessionClient::new(root.path().join("control.sock"))
                .query("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"),
            QueryOutcome::Unreachable(_)
        ));
    }
}
