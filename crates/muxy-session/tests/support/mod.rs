use muxy_proto::session::{
    CreateSessionRequest, EnvironmentEntry, SessionId, SessionOwner, WindowSize, WorkspacePlacement,
};
use muxy_session::{RendererClient, RendererEvent, SessionClient, current_build_mode};
use std::cell::RefCell;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct TestDaemon {
    pub root: tempfile::TempDir,
    pub socket: PathBuf,
    child: Option<Child>,
    startup_client: RefCell<Option<SessionClient>>,
}

impl TestDaemon {
    pub fn start() -> Self {
        let root = tempfile::Builder::new()
            .prefix("p8-isolated-test-")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let socket = root.path().join("control.sock");
        let mut child = spawn_isolated_daemon(&socket);
        let deadline = Instant::now() + Duration::from_secs(3);
        let startup_client = loop {
            assert!(
                child.try_wait().unwrap().is_none(),
                "isolated daemon exited before accepting a client"
            );
            assert!(
                Instant::now() < deadline,
                "daemon did not accept a client before the startup deadline"
            );
            match SessionClient::connect(&socket, current_build_mode()) {
                Ok(mut client) => {
                    if client.ping().is_ok() {
                        break client;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound
                            | io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::ConnectionReset
                    ) => {}
                Err(error) => panic!("isolated daemon startup connection failed: {error}"),
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        Self {
            root,
            socket,
            child: Some(child),
            startup_client: RefCell::new(Some(startup_client)),
        }
    }

    pub fn client(&self) -> SessionClient {
        let mut client =
            self.startup_client.borrow_mut().take().unwrap_or_else(|| {
                SessionClient::connect(&self.socket, current_build_mode()).unwrap()
            });
        client.ping().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        client
    }

    pub fn release_startup_client(&self) {
        self.startup_client.borrow_mut().take();
    }

    pub fn request(&self, label: &str) -> CreateSessionRequest {
        let resources = self.root.path().join("resources");
        let terminfo = self.root.path().join("terminfo");
        std::fs::create_dir_all(resources.join("shell-integration")).unwrap();
        std::fs::create_dir_all(&terminfo).unwrap();
        CreateSessionRequest {
            session_id: SessionId::new(),
            owner: SessionOwner {
                project_id: format!("project-{label}"),
                worktree_id: format!("worktree-{label}"),
                original_tab_id: format!("tab-{label}"),
            },
            placement: Some(WorkspacePlacement {
                project_id: format!("project-{label}"),
                worktree_id: format!("worktree-{label}"),
                tab_id: format!("tab-{label}"),
                area_id: format!("area-{label}"),
            }),
            working_directory: self.root.path().to_string_lossy().into_owned(),
            initial_size: WindowSize::new(80, 24),
            shell_executable: "/bin/sh".into(),
            argv: Vec::new(),
            startup_command: None,
            keep_shell_open: false,
            environment: vec![
                EnvironmentEntry {
                    key: "HOME".into(),
                    value: self.root.path().to_string_lossy().into_owned(),
                },
                EnvironmentEntry {
                    key: "PATH".into(),
                    value: "/usr/bin:/bin".into(),
                },
            ],
            ghostty_resources: resources.to_string_lossy().into_owned(),
            terminfo: terminfo.to_string_lossy().into_owned(),
            terminal_type: "xterm-ghostty".into(),
            color_terminal: "truecolor".into(),
            title: label.into(),
        }
    }

    pub fn finish(mut self, wait_for_idle: bool) {
        self.cleanup_sessions();
        self.startup_client.get_mut().take();
        if wait_for_idle {
            let deadline = Instant::now() + Duration::from_secs(12);
            let child = self.child.as_mut().unwrap();
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "isolated daemon exited unsuccessfully");
                    self.child = None;
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "isolated daemon did not exit idle"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        self.stop_child();
    }

    fn cleanup_sessions(&self) {
        let mut startup_client = self.startup_client.borrow_mut();
        if let Some(client) = startup_client.as_mut() {
            let _ = client.set_read_timeout(Some(Duration::from_secs(3)));
            client.end_all_sessions().unwrap();
            return;
        }
        drop(startup_client);
        if let Ok(mut client) = SessionClient::connect(&self.socket, current_build_mode()) {
            let _ = client.set_read_timeout(Some(Duration::from_secs(3)));
            client.end_all_sessions().unwrap();
        }
    }

    fn stop_child(&mut self) {
        if let Some(mut child) = self.child.take() {
            if child.try_wait().unwrap().is_none() {
                child.kill().unwrap();
            }
            child.wait().unwrap();
        }
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        self.cleanup_sessions();
        self.startup_client.get_mut().take();
        self.stop_child();
    }
}

pub fn spawn_isolated_daemon(socket: &Path) -> Child {
    let helper = env!("CARGO_BIN_EXE_muxy-session");
    let mut command = Command::new(helper);
    command
        .arg("daemon")
        .arg("--socket")
        .arg(socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn().unwrap()
}

pub fn renderer(socket: &Path, session_id: SessionId, generation: u64) -> RendererClient {
    let renderer = RendererClient::connect(
        socket,
        current_build_mode(),
        muxy_proto::session::AttachRequest {
            session_id,
            attachment_generation: generation,
            size: WindowSize::new(80, 24),
        },
    )
    .unwrap();
    renderer
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    renderer
}

pub fn read_until(renderer: &mut RendererClient, needle: &[u8]) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut bytes = Vec::new();
    while Instant::now() < deadline {
        match renderer.next_event() {
            Ok(Some(RendererEvent::Output { bytes: output, .. })) => {
                bytes.extend_from_slice(&output);
                if bytes.windows(needle.len()).any(|window| window == needle) {
                    return bytes;
                }
            }
            Ok(Some(RendererEvent::Exited(_))) | Ok(None) => break,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => panic!("renderer read failed: {error}"),
        }
    }
    panic!("renderer output did not contain {:?}: {:?}", needle, bytes)
}
