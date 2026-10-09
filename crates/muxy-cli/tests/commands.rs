mod support;

use muxy_client::Client;
use muxy_protocol::OperationId;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Profile(PathBuf);
impl Profile {
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("muxy-cli-{}", OperationId::new()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(support::binary());
        command
            .env_remove("MUXY_SERVER_BIN")
            .env("MUXY_DIR", &self.0)
            .env("MUXY_REMOTE_BIND", "127.0.0.1")
            .args(args);
        command
    }

    fn run(&self, args: &[&str]) -> Result<Output> {
        Ok(self.command(args).output()?)
    }
    fn socket(&self) -> PathBuf {
        self.0.join("server.sock")
    }
}

#[test]
fn linked_client_finds_its_sibling_server_and_reuses_a_server_when_the_sibling_is_missing() -> Result
{
    use std::os::unix::fs::symlink;
    let profile = Profile::new()?;
    let package = profile.0.join("installed pair");
    fs::create_dir(&package)?;
    let client = package.join("muxy");
    let server = package.join("muxy-server");
    copy_executable(&support::binary(), &client)?;
    let link = profile.0.join("muxy");
    symlink(&client, &link)?;
    let invoke = || {
        Command::new(&link)
            .env_remove("MUXY_SERVER_BIN")
            .env("MUXY_DIR", &profile.0)
            .args(["project", "list"])
            .output()
    };
    let missing = invoke()?;
    assert!(!missing.status.success());
    assert!(String::from_utf8(missing.stderr)?.contains(&server.display().to_string()));
    copy_executable(&support::binary().with_file_name("muxy-server"), &server)?;
    let launched = invoke()?;
    assert!(launched.status.success(), "{launched:?}");
    let first = Client::connect(&profile.socket())?;
    fs::rename(&server, package.join("running-server"))?;
    let reused = invoke()?;
    assert!(reused.status.success(), "{reused:?}");
    assert_eq!(launched.stdout, reused.stdout);
    assert_eq!(
        first.server_info(),
        Client::connect(&profile.socket())?.server_info()
    );
    Ok(())
}

fn copy_executable(source: &std::path::Path, destination: &std::path::Path) -> Result {
    // A separate process prevents concurrent test forks inheriting a writable
    // destination descriptor and temporarily blocking Linux exec with ETXTBSY.
    let output = Command::new("cp").args([source, destination]).output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}
impl Drop for Profile {
    fn drop(&mut self) {
        if let Ok(client) = Client::connect_with_timeout(&self.socket(), Duration::from_millis(100))
        {
            let _ = client.stop_server();
            let deadline = Instant::now() + Duration::from_secs(3);
            while self.socket().exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn simultaneous_client_startup_reuses_one_server_instance() -> Result {
    let profile = Profile::new()?;
    let socket = profile.socket();
    let instances = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    muxy_client::local::ensure_running(
                        &socket,
                        &support::binary().with_file_name("muxy-server"),
                    )
                    .map(|client| client.server_info().instance)
                })
            })
            .collect();
        workers
            .into_iter()
            .map(std::thread::ScopedJoinHandle::join)
            .collect::<Vec<_>>()
    });
    let mut ids = Vec::new();
    for result in instances {
        ids.push(result.map_err(|_| "startup thread panicked")??);
    }
    assert!(ids.iter().all(|id| *id == ids[0]));
    assert!(Client::connect(&socket)?.list_sessions()?.is_empty());
    Ok(())
}

#[test]
fn mobile_commands_enable_pair_revoke_and_disable() -> Result {
    use std::io::{BufRead, BufReader, Read};
    use std::process::Stdio;

    let profile = Profile::new()?;
    let port = std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port()
        .to_string();
    let enabled = profile.run(&["mobile", "enable", "--port", &port])?;
    assert!(enabled.status.success(), "{enabled:?}");
    assert!(String::from_utf8(enabled.stdout)?.contains(&format!("listening on port {port}")));

    let mut pairing = profile
        .command(&["mobile", "pair"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut output = BufReader::new(pairing.stdout.take().ok_or("no stdout")?);
    let (sender, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        while output.read_line(&mut line).is_ok_and(|read| read > 0) {
            if sender.send(std::mem::take(&mut line)).is_err() {
                break;
            }
        }
    });
    let link = loop {
        let line = lines.recv_timeout(Duration::from_secs(10))?;
        if line.starts_with("muxy://pair?") {
            break line.trim().to_owned();
        }
    };
    let mut invite = muxy_protocol::PairingInvite::parse_link(&link)
        .map_err(|code| format!("invalid link {link}: {code:?}"))?;
    invite.hosts = vec!["127.0.0.1".into()];
    let (_phone, paired) = Client::pair(&invite, "Test phone")?;
    let status = pairing.wait()?;
    let mut rest = String::new();
    while let Ok(line) = lines.recv_timeout(Duration::from_secs(1)) {
        rest.push_str(&line);
    }
    let mut errors = String::new();
    pairing
        .stderr
        .take()
        .ok_or("no stderr")?
        .read_to_string(&mut errors)?;
    assert!(status.success(), "{rest}{errors}");
    assert!(rest.contains("Paired Test phone."), "{rest}");

    let listed = String::from_utf8(profile.run(&["mobile"])?.stdout)?;
    assert!(listed.contains("Test phone  connected"), "{listed}");
    let device = paired.credential.device.to_string();
    let revoked = profile.run(&["mobile", "revoke", &device[..8]])?;
    assert!(revoked.status.success(), "{revoked:?}");
    assert!(String::from_utf8(profile.run(&["mobile"])?.stdout)?.contains("No paired devices."));
    let disabled = profile.run(&["mobile", "disable"])?;
    assert!(String::from_utf8(disabled.stdout)?.contains("Mobile access: off"));
    Ok(())
}
