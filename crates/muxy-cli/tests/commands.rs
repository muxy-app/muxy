mod support;

use muxy_client::Client;
use muxy_protocol::OperationId;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Profile(PathBuf);
impl Profile {
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("muxy-cli-{}", OperationId::new()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn socket(&self) -> PathBuf {
        self.0.join("server.sock")
    }
}

#[test]
fn linked_client_finds_its_sibling_server_and_reuses_a_server_when_the_sibling_is_missing() -> Result
{
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_file as symlink;
    let profile = Profile::new()?;
    let package = profile.0.join("installed pair");
    fs::create_dir(&package)?;
    let client = program(&package, "muxy");
    copy_executable(&support::binary(), &client)?;
    let server = program(&muxy_core::files::canonicalize(&package)?, "muxy-server");
    let link = program(&profile.0, "muxy");
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
    copy_executable(&support::server(), &server)?;
    let launched = invoke()?;
    assert!(launched.status.success(), "{launched:?}");
    let first = Client::connect(&profile.socket())?;
    fs::rename(&server, program(&package, "running-server"))?;
    let reused = invoke()?;
    assert!(reused.status.success(), "{reused:?}");
    assert_eq!(launched.stdout, reused.stdout);
    assert_eq!(
        first.server_info(),
        Client::connect(&profile.socket())?.server_info()
    );
    Ok(())
}

/// The program `name` in `directory`.
fn program(directory: &Path, name: &str) -> PathBuf {
    muxy_core::executable::sibling(&directory.join(name), name)
}

fn copy_executable(source: &Path, destination: &Path) -> Result {
    // A separate process prevents concurrent test forks inheriting a writable
    // destination descriptor and temporarily blocking Linux exec with ETXTBSY.
    #[cfg(unix)]
    {
        let output = Command::new("cp").args([source, destination]).output()?;
        assert!(output.status.success(), "{output:?}");
    }
    #[cfg(windows)]
    fs::copy(source, destination)?;
    Ok(())
}
impl Drop for Profile {
    fn drop(&mut self) {
        if let Ok(client) = Client::connect_with_timeout(&self.socket(), Duration::from_millis(100))
        {
            let _ = client.stop_server();
            let deadline = Instant::now() + Duration::from_secs(3);
            while muxy_core::files::FileId::of_link(&self.socket()).is_ok()
                && Instant::now() < deadline
            {
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
                    muxy_client::local::ensure_running(&socket, &support::server())
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
