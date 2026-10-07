use super::*;
use muxy_protocol::{SandboxPolicy, SandboxSettings, SandboxSpec};
use muxy_terminal::pty::{PtyEvent, PtySize};
use std::sync::mpsc;

#[test]
fn environment_requires_explicit_credential_names() {
    let mut sandbox = SandboxInfo {
        spec: SandboxSpec {
            workspace: ServerPath(b"/work".to_vec()),
            policy: SandboxPolicy::default(),
        },
        backend_version: NONO_VERSION.into(),
    };
    let request = SpawnRequest {
        clear_env: false,
        program: "/bin/zsh".into(),
        args: Vec::new(),
        cwd: "/work".into(),
        env: [
            ("TERM", "xterm"),
            ("TEST_API_KEY", "fixture"),
            ("HOME", "/host"),
            ("SSH_AUTH_SOCK", "/socket"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect(),
        size: PtySize { cols: 80, rows: 24 },
    };
    assert_eq!(
        environment(&request, &sandbox),
        vec![("TERM".into(), "xterm".into())]
    );
    sandbox.spec.policy.environment.push("TEST_API_KEY".into());
    assert_eq!(environment(&request, &sandbox).len(), 2);
}

#[test]
#[ignore = "requires macOS sandbox permission and MUXY_TEST_NONO pointing to nono 0.79.0"]
fn real_sandbox_enforces_policy_and_preserves_terminal() -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::var_os("MUXY_TEST_NONO").ok_or("set MUXY_TEST_NONO")?;
    let fixture = Runtime::new()?;
    let workspace = fixture.directory("workspace")?;
    let outside = fixture.directory("outside")?;
    fs::write(outside.join("secret"), "private-fixture")?;
    std::os::unix::fs::symlink(&outside, workspace.join("escape"))?;
    let settings = ServerSettings {
        sandbox: Some(SandboxSettings {
            executable: Some(ServerPath(executable.as_bytes().to_vec())),
            ..Default::default()
        }),
        ..Default::default()
    };
    for network in [SandboxNetwork::Blocked, SandboxNetwork::Domains] {
        let sandbox = super::super::info(SandboxSpec {
            workspace: ServerPath(workspace.as_os_str().as_bytes().to_vec()),
            policy: SandboxPolicy {
                network,
                domains: if network == SandboxNetwork::Domains {
                    vec!["example.com".into()]
                } else {
                    Vec::new()
                },
                ..Default::default()
            },
        })?;
        let request = SpawnRequest {
            clear_env: false,
            program: "/bin/zsh".into(),
            args: Vec::new(),
            cwd: workspace.clone(),
            env: vec![
                ("TERM".into(), "xterm-256color".into()),
                ("UNAPPROVED_SECRET".into(), "must-not-inherit".into()),
            ],
            size: PtySize { cols: 80, rows: 24 },
        };
        let mut pty = spawn(request, &settings, &sandbox)?;
        let shell = pty.shell_pid();
        assert!(process(shell).is_some());
        let (sender, receiver) = mpsc::channel();
        let reader = pty.start_reader(sender)?;
        pty.resize(PtySize {
            cols: 103,
            rows: 41,
        })?;
        pty.write(b"unsetopt zle; stty -echo; test -z \"$UNAPPROVED_SECRET\" && touch allowed && ! cat escape/secret && ! touch escape/created && /bin/sh -c '! cat escape/secret' && stty size && printf '\\nPOLICY_%s\\n' OK\n")?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();
        while Instant::now() < deadline {
            if let Ok(PtyEvent::Output(bytes)) = receiver.recv_timeout(Duration::from_millis(100)) {
                output.extend_from_slice(&bytes);
                if String::from_utf8_lossy(&output).contains("POLICY_OK\r\n") {
                    break;
                }
            }
        }
        let output = String::from_utf8_lossy(&output);
        assert!(output.contains("POLICY_OK\r\n"), "{network:?}: {output}");
        assert!(output.contains("41 103"), "{output}");
        assert!(workspace.join("allowed").exists());
        assert!(!outside.join("created").exists());
        assert_no_inherited_socket(shell)?;
        check_network(&mut pty, &receiver, network)?;
        check_job_control(&mut pty, &receiver)?;
        pty.write(b"printf '%s' \"$HOME\" > session-home; exit 23\n")?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = pty.try_wait() {
                break status;
            }
            assert!(Instant::now() < deadline, "shell did not exit");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code, Some(23));
        let home = PathBuf::from(fs::read_to_string(workspace.join("session-home"))?);
        drop(pty);
        drop(reader);
        assert!(process(shell).is_none_or(|info| info.pbi_status == 5));
        assert!(!home.exists(), "session home must be removed");
    }
    Ok(())
}

#[test]
fn failed_launch_never_creates_an_ordinary_session() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Runtime::new()?;
    let (sender, _) = mpsc::channel();
    let registry = crate::Registry::new(ServerSettings::default(), sender);
    let spec = SandboxSpec {
        workspace: ServerPath(fixture.directory.as_os_str().as_bytes().to_vec()),
        policy: SandboxPolicy::default(),
    };
    let operation = muxy_protocol::OperationId::new();
    let project = registry.home_project();
    let size = muxy_protocol::Size { cols: 80, rows: 24 };
    let error = registry
        .create_with_sandbox(
            project,
            operation,
            &fixture.directory,
            size,
            None,
            None,
            Some(spec),
        )
        .expect_err("missing backend must fail");
    assert_eq!(error.code(), ErrorCode::SpawnFailed);
    assert!(registry.list().is_empty());
    assert!(
        registry
            .create_project_session(project, operation, &fixture.directory, size)
            .is_err()
    );
    assert!(registry.list().is_empty());
    Ok(())
}

#[test]
fn older_settings_writes_preserve_sandbox_configuration() -> Result<(), Box<dyn std::error::Error>>
{
    let (sender, _) = mpsc::channel();
    let sandbox = SandboxSettings {
        executable: Some(ServerPath(b"/opt/nono".to_vec())),
        ..Default::default()
    };
    let registry = crate::Registry::new(
        ServerSettings {
            sandbox: Some(sandbox.clone()),
            ..Default::default()
        },
        sender,
    );
    registry.write_settings(ServerSettings::default())?;
    assert_eq!(registry.settings().sandbox, Some(sandbox));
    registry.write_settings(ServerSettings {
        sandbox: Some(SandboxSettings::default()),
        ..Default::default()
    })?;
    assert_eq!(
        registry.settings().sandbox,
        Some(SandboxSettings::default())
    );
    Ok(())
}

fn read_until(receiver: &mpsc::Receiver<PtyEvent>, marker: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut output = Vec::new();
    while Instant::now() < deadline {
        if let Ok(PtyEvent::Output(bytes)) = receiver.recv_timeout(Duration::from_millis(100)) {
            output.extend_from_slice(&bytes);
            if String::from_utf8_lossy(&output).contains(marker) {
                return String::from_utf8_lossy(&output).into_owned();
            }
        }
    }
    panic!("missing {marker}: {}", String::from_utf8_lossy(&output));
}

fn wait_foreground(pty: &Pty) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if pty
            .foreground_pid()
            .is_some_and(|pid| pid != pty.shell_pid())
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("shell never started its foreground job");
}

fn assert_no_inherited_socket(shell: u32) -> Result<(), Box<dyn std::error::Error>> {
    use libproc::file_info::{ListFDs, ProcFDType};
    use libproc::proc_pid::listpidinfo;
    let info = process(shell).ok_or("shell exited")?;
    let fds = listpidinfo::<ListFDs>(i32::try_from(shell)?, info.pbi_nfiles as usize)
        .map_err(std::io::Error::other)?;
    assert!(
        fds.iter()
            .all(|fd| !matches!(ProcFDType::from(fd.proc_fdtype), ProcFDType::Socket)),
        "shell inherited a supervisor socket"
    );
    Ok(())
}

fn check_job_control(
    pty: &mut Pty,
    receiver: &mpsc::Receiver<PtyEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    pty.write(b"sleep 30\n")?;
    wait_foreground(pty);
    pty.write(b"\x1a")?;
    read_until(receiver, "suspended");
    pty.write(b"fg\n")?;
    wait_foreground(pty);
    pty.write(b"\x03")?;
    pty.write(b"printf '\\nJOB_%s\\n' OK\n")?;
    read_until(receiver, "JOB_OK\r\n");
    Ok(())
}

struct ControlSocket(PathBuf);
impl Drop for ControlSocket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn check_network(
    pty: &mut Pty,
    receiver: &mpsc::Receiver<PtyEvent>,
    network: SandboxNetwork,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::net::TcpListener;
    use std::os::unix::net::UnixListener;
    let tcp = TcpListener::bind("127.0.0.1:0")?;
    let socket = ControlSocket(PathBuf::from("/private/tmp").join(format!(
        "muxy-control-{}",
        muxy_protocol::OperationId::new()
    )));
    let unix = UnixListener::bind(&socket.0)?;
    tcp.set_nonblocking(true)?;
    unix.set_nonblocking(true)?;
    let command = format!(
        "! nc -z -w 1 127.0.0.1 {} && ! nc -z -w 1 -U '{}' && ! curl -fsS --max-time 5 https://www.iana.org/ >/dev/null && ! curl -fsS --noproxy '*' --max-time 5 https://example.com/ >/dev/null && printf '\\nNETWORK_%s\\n' OK\n",
        tcp.local_addr()?.port(),
        socket.0.display()
    );
    pty.write(command.as_bytes())?;
    read_until(receiver, "NETWORK_OK\r\n");
    assert_eq!(
        tcp.accept().expect_err("host TCP must be denied").kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(
        unix.accept()
            .expect_err("host control socket must be denied")
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    if network == SandboxNetwork::Domains {
        pty.write(b"curl -fsS --max-time 10 https://example.com/ >/dev/null && printf '\\nHTTPS_%s\\n' OK\n")?;
        read_until(receiver, "HTTPS_OK\r\n");
    }
    Ok(())
}
