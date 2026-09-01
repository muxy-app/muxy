use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use muxy_proto::session::{
    AttachExisting, AttachRequest, Empty, EnvironmentEntry, HEADER_BYTES, LaunchSpecification,
    MAX_ENVIRONMENT_ENTRIES, MAX_FRAME_PAYLOAD_BYTES, MessageKind, OwnerMetadata, QueryResult,
    Resize, SESSION_MAGIC, SESSION_PROTOCOL_VERSION, SessionCodec, SessionMessage, SessionQuery,
    TerminationOutcome,
};
use tempfile::TempDir;
use uuid::Uuid;

const TIMEOUT: Duration = Duration::from_secs(8);

struct Daemon {
    child: Child,
    socket: PathBuf,
}

impl Daemon {
    fn start(root: &Path) -> Self {
        let socket = root.join("s/control.sock");
        let child = Command::new(binary())
            .arg("daemon")
            .arg("--socket")
            .arg(&socket)
            .arg("--idle-ms")
            .arg("200")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        wait_for(&socket, true);
        Self { child, socket }
    }

    fn connect(&self) -> UnixStream {
        let stream = UnixStream::connect(&self.socket).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(TIMEOUT)).unwrap();
        stream
    }

    fn request(&self, message: SessionMessage) -> SessionMessage {
        let mut stream = self.connect();
        write_message(&mut stream, &message);
        read_message(&mut stream)
    }

    fn attach(&self, request: AttachRequest) -> AttachClient {
        let mut stream = self.connect();
        write_message(&mut stream, &SessionMessage::AttachCreateOrAttach(request));
        let attached = read_message(&mut stream);
        let replay = read_message(&mut stream);
        assert!(
            matches!(attached, SessionMessage::Attached(_)),
            "unexpected attach response: {attached:?}"
        );
        let SessionMessage::Replay(replay) = replay else {
            panic!("missing replay")
        };
        AttachClient { stream, replay }
    }

    fn attach_existing(&self, session_id: &str) -> AttachClient {
        let mut stream = self.connect();
        write_message(
            &mut stream,
            &SessionMessage::AttachExisting(AttachExisting {
                session_id: session_id.to_owned(),
                size: size(80, 24),
            }),
        );
        let attached = read_message(&mut stream);
        let replay = read_message(&mut stream);
        assert!(
            matches!(attached, SessionMessage::Attached(_)),
            "unexpected attach response: {attached:?}"
        );
        let SessionMessage::Replay(replay) = replay else {
            panic!("missing replay")
        };
        AttachClient { stream, replay }
    }

    fn terminate_all(&self) -> Option<TerminationOutcome> {
        match self.request(SessionMessage::TerminateAll(Empty {})) {
            SessionMessage::TerminateResult(result) => Some(result.outcome),
            _ => None,
        }
    }

    fn wait_for_exit(&mut self) {
        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            if self.child.try_wait().unwrap().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("daemon did not exit before the deadline");
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if self.socket.exists() {
            let _ = self.terminate_all();
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct AttachClient {
    stream: UnixStream,
    replay: Vec<u8>,
}

impl AttachClient {
    fn disable_echo(&mut self) {
        self.input(b"stty -echo\n");
        std::thread::sleep(Duration::from_millis(200));
    }

    fn input(&mut self, bytes: &[u8]) {
        write_message(&mut self.stream, &SessionMessage::Input(bytes.to_vec()));
    }

    fn resize(&mut self, columns: u16, rows: u16) {
        write_message(
            &mut self.stream,
            &SessionMessage::Resize(size(columns, rows)),
        );
    }

    fn output_until(&mut self, marker: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            match read_message(&mut self.stream) {
                SessionMessage::Output(bytes) | SessionMessage::Replay(bytes) => {
                    output.extend(bytes);
                    if contains(&output, marker) {
                        return output;
                    }
                }
                SessionMessage::Exited(status) => panic!("session exited early: {status:?}"),
                SessionMessage::ProtocolError(error) => panic!("protocol error: {error:?}"),
                _ => {}
            }
        }
        panic!(
            "output marker was not observed: {:?}",
            String::from_utf8_lossy(marker)
        )
    }

    fn output_and_exit_status(&mut self) -> (Vec<u8>, muxy_proto::session::ExitStatus) {
        let mut output = Vec::new();
        loop {
            match read_message(&mut self.stream) {
                SessionMessage::Replay(bytes) | SessionMessage::Output(bytes) => {
                    output.extend(bytes);
                }
                SessionMessage::Exited(status) => return (output, status),
                SessionMessage::ProtocolError(error) => panic!("protocol error: {error:?}"),
                _ => {}
            }
        }
    }
}

#[test]
fn daemon_replays_detached_output_replaces_clients_resizes_and_exits_idle() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let mut daemon = Daemon::start(&root);
    let session_id = canonical_id();
    let mut first = daemon.attach(request(&session_id, &root, Some("printf 'START_ONCE\\n'")));
    let startup = first.output_until(b"START_ONCE");
    assert_eq!(count(&startup, b"START_ONCE"), 1);
    first.disable_echo();
    first.input(b"sleep 0.2; printf 'DETACHED_OUTPUT\\n'\n");
    drop(first);
    std::thread::sleep(Duration::from_millis(400));

    let mut second = daemon.attach_existing(&session_id);
    assert!(contains(&second.replay, b"START_ONCE"));
    assert!(contains(&second.replay, b"DETACHED_OUTPUT"));
    assert_eq!(count(&second.replay, b"START_ONCE"), 1);
    second.input(b"printf 'LIVE_OUTPUT\\n'\n");
    second.output_until(b"LIVE_OUTPUT");
    second.resize(93, 41);
    second.input(b"stty size\n");
    let resized = second.output_until(b"41 93");
    assert!(
        contains(&resized, b"41 93"),
        "resize output: {}",
        String::from_utf8_lossy(&resized)
    );

    let mut replacement = daemon.attach_existing(&session_id);
    replacement.input(b"printf 'REPLACED_CLIENT\\n'\n");
    replacement.output_until(b"REPLACED_CLIENT");
    assert!(matches!(
        daemon.request(SessionMessage::Query(SessionQuery {
            session_id: session_id.clone(),
        })),
        SessionMessage::QueryResult(QueryResult::Found(_))
    ));
    assert_eq!(daemon.terminate_all(), Some(TerminationOutcome::Terminated));
    drop(replacement);
    drop(second);
    wait_for(&daemon.socket, false);
    daemon.wait_for_exit();
}

#[test]
fn replay_is_truncated_malformed_connections_are_isolated_and_floods_do_not_kill_sessions() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let daemon = Daemon::start(&root);
    let session_id = canonical_id();
    let mut client = daemon.attach(request(&session_id, &root, None));
    client.disable_echo();
    client.input(b"yes 0123456789abcdef | head -n 30000; printf 'TAIL_MARKER\\n'; sleep 0.2\n");
    let flooded = client.output_until(b"TAIL_MARKER");
    assert!(
        flooded.len() > 300_000,
        "flooded output length: {}",
        flooded.len()
    );
    drop(client);
    std::thread::sleep(Duration::from_millis(300));
    let replayed = daemon.attach_existing(&session_id);
    assert_eq!(replayed.replay.len(), 256 * 1024);
    assert!(contains(&replayed.replay, b"TAIL_MARKER"));
    drop(replayed);

    let mut malformed = daemon.connect();
    malformed
        .write_all(b"BAD!\x00\x02\x00\x01\x00\x00\x00\x00")
        .unwrap();
    malformed.shutdown(std::net::Shutdown::Write).unwrap();
    let mut rejection = Vec::new();
    malformed.read_to_end(&mut rejection).unwrap();
    assert!(matches!(
        daemon.request(SessionMessage::Query(SessionQuery {
            session_id: session_id.clone(),
        })),
        SessionMessage::QueryResult(QueryResult::Found(_))
    ));

    let mut slow = daemon.attach_existing(&session_id);
    slow.input(b"head -c 5000000 /dev/zero | tr '\\000' y\n");
    std::thread::sleep(Duration::from_millis(500));
    assert!(matches!(
        daemon.request(SessionMessage::Query(SessionQuery { session_id })),
        SessionMessage::QueryResult(QueryResult::Found(_))
    ));
}

#[test]
fn environment_input_and_connection_floods_are_bounded_without_daemon_loss() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let daemon = Daemon::start(&root);
    let session_id = canonical_id();
    let mut launch = request(&session_id, &root, Some("printf 'ENVIRONMENT_READY\\n'"));
    let path = launch.launch.environment.remove(0);
    launch.launch.environment = std::iter::once(path)
        .chain((1..MAX_ENVIRONMENT_ENTRIES).map(|index| EnvironmentEntry {
            key: format!("FLOOD_{index}"),
            value: "x".repeat(64),
        }))
        .collect();
    let mut client = daemon.attach(launch);
    client.output_until(b"ENVIRONMENT_READY");
    let mut oversized_input = Vec::with_capacity(HEADER_BYTES);
    oversized_input.extend_from_slice(&SESSION_MAGIC);
    oversized_input.extend_from_slice(&SESSION_PROTOCOL_VERSION.to_be_bytes());
    oversized_input.extend_from_slice(&(MessageKind::Input as u16).to_be_bytes());
    oversized_input.extend_from_slice(&((MAX_FRAME_PAYLOAD_BYTES + 1) as u32).to_be_bytes());
    client.stream.write_all(&oversized_input).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert!(matches!(
        daemon.request(SessionMessage::Query(SessionQuery {
            session_id: session_id.clone(),
        })),
        SessionMessage::QueryResult(QueryResult::Found(_))
    ));
    drop(client);

    let mut held = Vec::new();
    for _ in 0..64 {
        let mut stream = daemon.connect();
        stream.write_all(b"M").unwrap();
        held.push(stream);
    }
    std::thread::sleep(Duration::from_millis(1_500));
    let mut overflow = daemon.connect();
    write_message(
        &mut overflow,
        &SessionMessage::Query(SessionQuery {
            session_id: session_id.clone(),
        }),
    );
    let mut byte = [0u8; 1];
    assert!(overflow.read(&mut byte).is_err() || byte[0] == 0);
    drop(overflow);
    drop(held);
    std::thread::sleep(Duration::from_millis(3_300));
    assert!(matches!(
        daemon.request(SessionMessage::Query(SessionQuery { session_id })),
        SessionMessage::QueryResult(QueryResult::Found(_))
    ));
}

#[test]
fn shell_exit_reports_status_removes_the_session_and_allows_idle_exit() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let mut daemon = Daemon::start(&root);
    let session_id = canonical_id();
    let mut client = daemon.attach(request(&session_id, &root, Some("printf 'SHELL_READY\\n'")));
    client.output_until(b"SHELL_READY");
    client.input(b"printf 'FINAL_OUTPUT\\n'; exit 23\n");
    let (output, status) = client.output_and_exit_status();
    assert!(contains(&output, b"FINAL_OUTPUT"));
    assert_eq!(status.code, Some(23));
    assert_eq!(status.signal, None);
    wait_for_missing(&daemon, &session_id);
    drop(client);
    wait_for(&daemon.socket, false);
    daemon.wait_for_exit();
}

#[test]
fn terminate_one_and_all_remove_foreground_background_and_term_resistant_members() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let daemon = Daemon::start(&root);
    let session_id = canonical_id();
    let (client, background_pid) = resistant_session(&daemon, &root, &session_id);
    assert!(matches!(
        daemon.request(SessionMessage::TerminateOne(SessionQuery {
            session_id: session_id.clone(),
        })),
        SessionMessage::TerminateResult(result)
            if result.outcome == TerminationOutcome::Terminated
    ));
    wait_for_process_exit(background_pid);
    wait_for_missing(&daemon, &session_id);
    drop(client);

    let mut clients = Vec::new();
    let mut session_ids = Vec::new();
    let mut background_pids = Vec::new();
    for _ in 0..2 {
        let session_id = canonical_id();
        let (client, background_pid) = resistant_session(&daemon, &root, &session_id);
        clients.push(client);
        session_ids.push(session_id);
        background_pids.push(background_pid);
    }
    assert_eq!(daemon.terminate_all(), Some(TerminationOutcome::Terminated));
    for background_pid in background_pids {
        wait_for_process_exit(background_pid);
    }
    for session_id in session_ids {
        wait_for_missing(&daemon, &session_id);
    }
    drop(clients);
}

fn resistant_session(daemon: &Daemon, root: &Path, session_id: &str) -> (AttachClient, i32) {
    let mut client = daemon.attach(request(session_id, root, Some("printf 'SHELL_READY\\n'")));
    client.output_until(b"SHELL_READY");
    client.input(
        b"sh -c 'trap \"\" TERM; while :; do sleep 1; done' & printf 'BACKGROUND:%s\\n' $!\n",
    );
    let output = client.output_until(b"BACKGROUND:");
    let background_pid = parse_pid_after(&output, b"BACKGROUND:").unwrap();
    assert_eq!(unsafe { libc::kill(background_pid, 0) }, 0);
    client.input(b"exec sh -c 'trap \"\" TERM; while :; do sleep 1; done'\n");
    std::thread::sleep(Duration::from_millis(200));
    (client, background_pid)
}

fn wait_for_process_exit(process_id: i32) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && unsafe { libc::kill(process_id, 0) } == 0 {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_ne!(unsafe { libc::kill(process_id, 0) }, 0);
}

fn wait_for_missing(daemon: &Daemon, session_id: &str) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if matches!(
            daemon.request(SessionMessage::Query(SessionQuery {
                session_id: session_id.to_owned(),
            })),
            SessionMessage::QueryResult(QueryResult::Missing)
        ) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("session did not disappear: {session_id}");
}

#[test]
fn concurrent_private_attach_processes_share_one_lazy_daemon_and_isolate_socket_roots() {
    let first_root = TempDir::new().unwrap();
    let second_root = TempDir::new().unwrap();
    let first_path = first_root.path().canonicalize().unwrap();
    let second_path = second_root.path().canonicalize().unwrap();
    let first_socket = first_path.join("sessions-v2-dev/control.sock");
    let second_socket = second_path.join("sessions-v2/control.sock");
    let mut first = spawn_attach(&first_socket, &canonical_id(), &first_path);
    let mut second = spawn_attach(&first_socket, &canonical_id(), &first_path);
    wait_for(&first_socket, true);
    let first_daemon = DaemonProxy {
        socket: first_socket.clone(),
    };
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if matches!(
            first_daemon.request(SessionMessage::Recover(Empty {})),
            SessionMessage::Recovered(descriptors) if descriptors.len() == 2
        ) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }

    let second_daemon = Daemon::start_with_socket(&second_socket);
    assert!(matches!(
        second_daemon.request(SessionMessage::Recover(Empty {})),
        SessionMessage::Recovered(descriptors) if descriptors.is_empty()
    ));
    let _ = first_daemon.request(SessionMessage::TerminateAll(Empty {}));
    let _ = first.wait();
    let _ = second.wait();
}

impl Daemon {
    fn start_with_socket(socket: &Path) -> Self {
        let child = Command::new(binary())
            .arg("daemon")
            .arg("--socket")
            .arg(socket)
            .arg("--idle-ms")
            .arg("200")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        wait_for(socket, true);
        Self {
            child,
            socket: socket.to_path_buf(),
        }
    }
}

struct DaemonProxy {
    socket: PathBuf,
}

impl DaemonProxy {
    fn request(&self, message: SessionMessage) -> SessionMessage {
        let mut stream = UnixStream::connect(&self.socket).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        write_message(&mut stream, &message);
        read_message(&mut stream)
    }
}

fn spawn_attach(socket: &Path, session_id: &str, root: &Path) -> Child {
    let resources = resources();
    let mut child = Command::new(binary());
    child
        .arg("attach")
        .env("MUXY_SESSION_SOCKET", socket)
        .env("MUXY_SESSION_ID", session_id)
        .env("MUXY_SESSION_PROJECT_ID", canonical_id())
        .env("MUXY_SESSION_TITLE", "process test")
        .env("MUXY_SESSION_SHELL", "/bin/sh")
        .env("MUXY_SESSION_RESOURCES", resources)
        .env("MUXY_SESSION_DIRECTORY", root)
        .env("MUXY_SESSION_CREATE_POLICY", "create-or-attach")
        .env("MUXY_SESSION_DAEMON_IDLE_MS", "200")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"printf 'ATTACH_READY\\n'; read line\n")
        .unwrap();
    child
}

fn request(session_id: &str, root: &Path, startup_command: Option<&str>) -> AttachRequest {
    AttachRequest {
        session_id: session_id.to_owned(),
        owner: OwnerMetadata {
            project_id: canonical_id(),
            worktree_id: Some(canonical_id()),
            title: "session process test".to_owned(),
        },
        launch: LaunchSpecification {
            shell: "/bin/sh".to_owned(),
            resources_directory: resources().to_string_lossy().into_owned(),
            working_directory: root.to_string_lossy().into_owned(),
            startup_command: Some(match startup_command {
                Some(command) => format!("stty -echo; {command}"),
                None => "stty -echo".to_owned(),
            }),
            environment: vec![EnvironmentEntry {
                key: "PATH".to_owned(),
                value: std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned()),
            }],
        },
        size: size(80, 24),
    }
}

fn size(columns: u16, rows: u16) -> Resize {
    Resize {
        columns,
        rows,
        width_px: u32::from(columns) * 8,
        height_px: u32::from(rows) * 16,
    }
}

fn binary() -> PathBuf {
    std::env::var_os("MUXY_TEST_SESSION_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_muxy-session-v2")))
}

fn resources() -> PathBuf {
    std::env::var_os("MUXY_TEST_SESSION_RESOURCES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

fn canonical_id() -> String {
    Uuid::new_v4().hyphenated().to_string().to_ascii_uppercase()
}

fn write_message(stream: &mut UnixStream, message: &SessionMessage) {
    stream
        .write_all(&SessionCodec::encode(message).unwrap())
        .unwrap();
}

fn read_message(stream: &mut UnixStream) -> SessionMessage {
    let mut header = [0u8; HEADER_BYTES];
    stream.read_exact(&mut header).unwrap();
    let length = u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;
    let mut frame = header.to_vec();
    frame.resize(HEADER_BYTES + length, 0);
    stream.read_exact(&mut frame[HEADER_BYTES..]).unwrap();
    SessionCodec::decode(&frame).unwrap()
}

fn wait_for(path: &Path, present: bool) {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if path.exists() == present {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("path state did not converge: {}", path.display());
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn parse_pid_after(bytes: &[u8], marker: &[u8]) -> Option<i32> {
    let start = bytes
        .windows(marker.len())
        .position(|window| window == marker)?
        + marker.len();
    let digits = bytes[start..]
        .iter()
        .copied()
        .take_while(u8::is_ascii_digit)
        .collect::<Vec<_>>();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

#[test]
fn aborted_connections_never_stop_the_daemon() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let daemon = Daemon::start(&root);
    let session_id = canonical_id();
    let mut client = daemon.attach(request(&session_id, &root, Some("printf 'SHELL_READY\\n'")));
    client.output_until(b"SHELL_READY");
    for _ in 0..500 {
        let _ = UnixStream::connect(&daemon.socket);
    }
    std::thread::sleep(Duration::from_millis(200));
    assert!(matches!(
        daemon.request(SessionMessage::Query(SessionQuery {
            session_id: session_id.clone(),
        })),
        SessionMessage::QueryResult(QueryResult::Found(_))
    ));
    drop(client);
}
