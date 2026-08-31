pub mod support;

use muxy_proto::session::{
    AttachRequest, CreateSessionRequest, EnvironmentEntry, ProcessIdentity, SessionId,
    SessionOwner, WindowSize, WorkspacePlacement,
};
use muxy_session::{
    RendererClient, SessionClient, current_build_mode, identity_is_alive, process_identity,
};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use support::read_until;

#[test]
fn staged_bundle_helper_detaches_survives_app_close_and_cleans() {
    if let Some(root) = std::env::var_os("P8_STAGED_CLEANUP_ROOT") {
        cleanup_recorded_processes(&PathBuf::from(root));
        return;
    }
    let Some(helper) = std::env::var_os("P8_STAGED_SESSION_HELPER") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("P8_STAGED_SESSION_ROOT").unwrap());
    let ready = PathBuf::from(std::env::var_os("P8_STAGED_READY_FILE").unwrap());
    let proceed = PathBuf::from(std::env::var_os("P8_STAGED_PROCEED_FILE").unwrap());
    assert_eq!(
        std::fs::read_to_string(root.join(".muxy-p8-owner"))
            .unwrap()
            .trim(),
        "muxy-p8-terminal-memory-v1"
    );
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let helper = PathBuf::from(helper);
    assert!(helper.is_absolute());
    assert_eq!(
        helper.file_name().and_then(|name| name.to_str()),
        Some("muxy-session")
    );
    let socket = root.join("control.sock");
    let mut daemon = spawn_daemon(&helper, &socket);
    let daemon_identity = process_identity(daemon.id()).unwrap();
    write_owned_process_records(&root, &[(daemon_identity, "daemon")]);
    wait_for_socket(&socket, &mut daemon);
    let mut control = SessionClient::connect(&socket, current_build_mode()).unwrap();
    let descriptor = control
        .create_session(request(&helper, &root, "staged-helper"))
        .unwrap();
    write_owned_processes(&root, descriptor.shell, daemon_identity);
    let mut first = renderer(&socket, descriptor.session_id, 1);
    first
        .send_input(b"stty -echo; printf 'P8_STAGED_REPLAY\\n'\n")
        .unwrap();
    read_until(&mut first, b"P8_STAGED_REPLAY");
    first.disconnect().unwrap();
    assert!(identity_is_alive(descriptor.shell));
    std::fs::write(&ready, b"ready\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !proceed.exists() {
        assert!(
            Instant::now() < deadline,
            "staged verifier did not continue"
        );
        assert!(identity_is_alive(descriptor.shell));
        assert!(identity_is_alive(daemon_identity));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(identity_is_alive(descriptor.shell));
    assert!(identity_is_alive(daemon_identity));
    let mut second = renderer(&socket, descriptor.session_id, 2);
    read_until(&mut second, b"P8_STAGED_REPLAY");
    second.send_input(b"printf 'P8_STAGED_LIVE\\n'\n").unwrap();
    read_until(&mut second, b"P8_STAGED_LIVE");
    second.disconnect().unwrap();
    control.end_session(descriptor.session_id).unwrap();
    assert!(!identity_is_alive(descriptor.shell));
    drop(control);
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Some(status) = daemon.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "staged daemon did not exit idle");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::fs::remove_file(root.join("owned-processes")).unwrap();
}

#[test]
fn staged_phase_five_session_identities_are_stable() {
    let Some(root) = std::env::var_os("P8_STAGED_PHASE5_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    validate_owned_root(&root);
    let socket = PathBuf::from(std::env::var_os("P8_STAGED_PHASE5_SOCKET").unwrap());
    let app_support = root.join("app");
    assert_eq!(
        socket.parent().and_then(Path::parent),
        Some(app_support.as_path())
    );
    let snapshot = root.join("phase5-session-identities");
    let mut client = SessionClient::connect(&socket, current_build_mode()).unwrap();
    let mut identities = client
        .list_sessions()
        .unwrap()
        .into_iter()
        .map(|session| (session.session_id, session.shell))
        .collect::<Vec<_>>();
    identities.sort_by_key(|(session_id, _)| *session_id);
    assert!(!identities.is_empty());
    for (_, shell) in &identities {
        assert!(identity_is_alive(*shell));
    }
    let contents = identities
        .iter()
        .map(|(session_id, shell)| {
            format!(
                "{session_id} {} {}\n",
                shell.process_id, shell.start_identity
            )
        })
        .collect::<String>();
    match std::env::var("P8_STAGED_PHASE5_MODE").as_deref() {
        Ok("snapshot") => std::fs::write(snapshot, contents).unwrap(),
        Ok("verify") => assert_eq!(std::fs::read_to_string(snapshot).unwrap(), contents),
        _ => panic!("invalid staged Phase 5 identity probe mode"),
    }
}

#[test]
fn staged_recorded_processes_are_dead() {
    let Some(root) = std::env::var_os("P8_STAGED_VERIFY_DEAD_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    validate_owned_root(&root);
    let records = read_owned_processes(&root);
    for (identity, _) in &records {
        assert!(!identity_is_alive(*identity));
    }
    std::fs::remove_file(root.join("owned-processes")).unwrap();
}

fn spawn_daemon(helper: &Path, socket: &Path) -> Child {
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
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn().unwrap()
}

fn wait_for_socket(socket: &Path, daemon: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !socket.exists() {
        assert!(
            daemon.try_wait().unwrap().is_none(),
            "staged helper daemon exited"
        );
        assert!(
            Instant::now() < deadline,
            "staged helper socket was not created"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn request(helper: &Path, root: &Path, label: &str) -> CreateSessionRequest {
    let resources = helper.parent().unwrap().parent().unwrap().join("Resources");
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
        working_directory: root.to_string_lossy().into_owned(),
        initial_size: WindowSize::new(80, 24),
        shell_executable: "/bin/sh".into(),
        argv: Vec::new(),
        startup_command: None,
        keep_shell_open: false,
        environment: vec![
            EnvironmentEntry {
                key: "HOME".into(),
                value: root.to_string_lossy().into_owned(),
            },
            EnvironmentEntry {
                key: "PATH".into(),
                value: "/usr/bin:/bin".into(),
            },
        ],
        ghostty_resources: resources.join("ghostty").to_string_lossy().into_owned(),
        terminfo: resources.join("terminfo").to_string_lossy().into_owned(),
        terminal_type: "xterm-ghostty".into(),
        color_terminal: "truecolor".into(),
        title: label.into(),
    }
}

fn renderer(socket: &Path, session_id: SessionId, generation: u64) -> RendererClient {
    let renderer = RendererClient::connect(
        socket,
        current_build_mode(),
        AttachRequest {
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

fn write_owned_processes(root: &Path, shell: ProcessIdentity, daemon: ProcessIdentity) {
    write_owned_process_records(root, &[(shell, "shell"), (daemon, "daemon")]);
}

fn write_owned_process_records(root: &Path, records: &[(ProcessIdentity, &str)]) {
    let contents = records
        .iter()
        .map(|(identity, role)| {
            format!(
                "{} {} {role}\n",
                identity.process_id, identity.start_identity
            )
        })
        .collect::<String>();
    std::fs::write(root.join("owned-processes"), contents).unwrap();
}

fn validate_owned_root(root: &Path) {
    let canonical_tmp = std::fs::canonicalize("/tmp").unwrap();
    assert_eq!(root.parent(), Some(canonical_tmp.as_path()));
    assert!(
        root.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("p8-isolated-test-"))
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".muxy-p8-owner"))
            .unwrap()
            .trim(),
        "muxy-p8-terminal-memory-v1"
    );
}

fn read_owned_processes(root: &Path) -> Vec<(ProcessIdentity, String)> {
    std::fs::read_to_string(root.join("owned-processes"))
        .unwrap()
        .lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            let identity = ProcessIdentity {
                process_id: fields.next().unwrap().parse().unwrap(),
                start_identity: fields.next().unwrap().parse().unwrap(),
            };
            let role = fields.next().unwrap().to_owned();
            assert!(matches!(role.as_str(), "shell" | "daemon"));
            assert!(fields.next().is_none());
            (identity, role)
        })
        .collect()
}

fn cleanup_recorded_processes(root: &Path) {
    validate_owned_root(root);
    for (identity, _) in read_owned_processes(root) {
        terminate_exact_identity(identity);
    }
    std::fs::remove_file(root.join("owned-processes")).unwrap();
}

fn terminate_exact_identity(identity: ProcessIdentity) {
    if !identity_is_alive(identity) {
        return;
    }
    let pid = libc::pid_t::try_from(identity.process_id).unwrap();
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    let deadline = Instant::now() + Duration::from_secs(2);
    while identity_is_alive(identity) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if !identity_is_alive(identity) {
        return;
    }
    assert!(identity_is_alive(identity));
    assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
    let deadline = Instant::now() + Duration::from_secs(2);
    while identity_is_alive(identity) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!identity_is_alive(identity));
}
