mod support;

use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use muxy_client::Client;
use muxy_protocol::{OperationId, SessionId, Size};
use serde_json::{Value, json};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    directory: tempfile::TempDir,
    profile: PathBuf,
}
impl Fixture {
    fn new() -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let profile = std::env::temp_dir().join(format!("muxy-m-{}", OperationId::new()));
        std::fs::create_dir(&profile)?;
        Ok(Self { directory, profile })
    }
    fn run(&self, args: &[impl AsRef<std::ffi::OsStr>]) -> Result<Output> {
        Ok(Command::new(support::binary())
            .args(args)
            .env("MUXY_DIR", &self.profile)
            .env_remove("MUXY_SERVER_BIN")
            .env_remove("MUXY_PANE_ID")
            .current_dir(self.directory.path())
            .output()?)
    }
    fn ok(&self, args: &[&str]) -> Result<String> {
        let result = self.run(args)?;
        assert!(result.status.success(), "{args:?}: {result:?}");
        Ok(String::from_utf8(result.stdout)?)
    }
    fn json(&self, args: &[&str]) -> Result<Value> {
        Ok(serde_json::from_str(&self.ok(args)?)?)
    }
    fn client(&self) -> Result<Client> {
        Ok(Client::connect(&self.profile.join("server.sock"))?)
    }
    fn project(&self) -> Result<String> {
        let value = self.json(&["project", "add", ".", "--name", "Test", "--json"])?;
        Ok(value["id"].as_str().ok_or("project ID")?.into())
    }
    fn shell(&self) -> Result {
        self.ok(&["settings", "set", "default-shell", "/bin/sh"])?;
        self.ok(&["settings", "set", "shell-integration", "false"])?;
        Ok(())
    }
    fn git(&self, args: &[&str]) -> Result {
        let result = Command::new("git")
            .args(args)
            .current_dir(self.directory.path())
            .output()?;
        assert!(result.status.success(), "{args:?}: {result:?}");
        Ok(())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let socket = self.profile.join("server.sock");
        if let Ok(client) = Client::connect_with_timeout(&socket, Duration::from_millis(100)) {
            let _ = client.stop_server();
            let deadline = Instant::now() + Duration::from_secs(5);
            while socket.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

#[test]
fn projects_settings_files_exec_and_activity_are_scriptable() -> Result {
    let f = Fixture::new()?;
    let project = f.project()?;
    f.ok(&["project", "rename", &project, "Renamed"])?;
    f.ok(&["project", "set-color", &project, "#123456"])?;
    f.ok(&["project", "set-icon", &project, "sf:terminal"])?;
    let listed = f.json(&["project", "list", "--json"])?;
    let found = listed
        .as_array()
        .ok_or("projects")?
        .iter()
        .find(|p| p["id"] == project)
        .ok_or("project")?;
    assert_eq!(found["name"], "Renamed");
    assert_eq!(found["color"], "#123456");
    assert_eq!(found["icon"], "sf:terminal");
    f.shell()?;
    assert_eq!(f.json(&["settings", "get"])?["default_shell"], "/bin/sh");
    assert!(
        !f.run(&["settings", "set", "history-budget-bytes", "999999999999999"])?
            .status
            .success()
    );
    let write = json!({"Write":{"path":b"file.txt", "content":"hello | world\n"}}).to_string();
    f.ok(&["files", &project, &write])?;
    let read = json!({"Read":b"file.txt"}).to_string();
    assert_eq!(
        f.json(&["files", &project, &read])?["Content"]["content"],
        "hello | world\n"
    );
    let escape = json!({"Read":b"../outside"}).to_string();
    assert!(!f.run(&["files", &project, &escape])?.status.success());
    assert_eq!(
        f.ok(&["exec", "Renamed", "--", "/usr/bin/printf", "%s", "--json"])?,
        "--json"
    );
    let failed = f.run(&[
        "exec",
        &project,
        "--json",
        "--",
        "/bin/sh",
        "-c",
        "printf failure; exit 7",
    ])?;
    assert!(!failed.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&failed.stdout)?["exit_code"],
        7
    );
    assert!(f.json(&["activity", "list"])?.is_object());
    f.ok(&["project", "delete", &project, "--yes"])?;
    assert!(f.directory.path().join("file.txt").exists());
    Ok(())
}

#[test]
fn sessions_survive_commands_accept_input_and_keep_saved_output() -> Result {
    let f = Fixture::new()?;
    let project = f.project()?;
    f.shell()?;
    let session = f
        .ok(&[
            "session", "create", &project, "--cols", "96", "--rows", "32",
        ])?
        .trim()
        .to_owned();
    let id = SessionId::new(session.parse()?).ok_or("session")?;
    let client = f.client()?;
    let unattended = f.json(&["session", "read-screen", &session, "--json"])?;
    assert_eq!(unattended["size"], json!({"cols":96,"rows":32}));
    let attachment = client.attach(id, Size { cols: 96, rows: 32 })?;
    f.ok(&[
        "session",
        "send",
        &session,
        "printf '\\nCLI_%s\\n' 'OUTPUT'",
    ])?;
    f.ok(&["session", "send-keys", &session, "Enter"])?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let screen = f.json(&["session", "read-screen", &session, "--json"])?;
        assert_eq!(screen["size"], json!({"cols":96,"rows":32}));
        if screen["text"]
            .as_str()
            .is_some_and(|text| text.contains("CLI_OUTPUT"))
        {
            break;
        }
        assert!(Instant::now() < deadline, "{screen}");
        std::thread::sleep(Duration::from_millis(20));
    }
    let sessions = f.json(&["session", "list", "--project", &project, "--json"])?;
    assert_eq!(sessions[0]["id"], session);
    assert_eq!(sessions[0]["status"], "Live");
    assert_eq!(sessions[0]["attached"], true);
    assert!(!f.run(&["server", "stop"])?.status.success());
    assert!(!f.run(&["session", "end", &session])?.status.success());
    let search = f.json(&["session", "search", &session, "CLI_OUTPUT"])?;
    assert!(!search["matches"].as_array().ok_or("matches")?.is_empty());
    assert!(f.json(&["session", "history", &session])?["rows"].is_array());
    client.detach(attachment.channel)?;
    f.ok(&["session", "end", &session, "--yes"])?;
    assert!(
        f.ok(&["session", "read-screen", &session, "--saved"])?
            .contains("CLI_OUTPUT")
    );
    assert_eq!(f.json(&["session", "list", "--json"])?, json!([]));
    assert_eq!(
        f.json(&["session", "list", "--project", &project, "--json"])?,
        json!([])
    );
    assert_eq!(
        f.json(&["session", "list", "--all", "--json"])?[0]["status"],
        "Ended"
    );
    assert_eq!(
        f.json(&["session", "list", "--project", &project, "--all", "--json"])?[0]["id"],
        session
    );
    f.ok(&["session", "discard", &session, "--yes"])?;
    assert_eq!(
        f.json(&["session", "list", "--project", &project, "--all", "--json"])?,
        json!([])
    );
    f.ok(&["server", "stop"])?;
    Ok(())
}

#[test]
fn worktrees_use_server_git_and_refuse_dirty_removal() -> Result {
    let f = Fixture::new()?;
    f.git(&["init", "-b", "main"])?;
    f.git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "--allow-empty",
        "-m",
        "initial",
    ])?;
    let project = f.project()?;
    let directory = f.directory.path().join("child");
    let path = directory.to_str().ok_or("path")?;
    let created = f.json(&[
        "worktree", "create", &project, path, "--branch", "feature", "--json",
    ])?;
    let worktree = created["id"].as_str().ok_or("worktree")?;
    assert!(directory.is_dir());
    assert!(
        f.json(&["worktree", "list", &project])?["Worktrees"]
            .as_array()
            .is_some_and(|rows| rows.len() == 2)
    );
    assert!(f.json(&["git", &project, "\"Summary\""])?["Summary"].is_object());
    std::fs::write(directory.join("untracked"), "keep")?;
    assert!(
        !f.run(&["worktree", "remove", worktree, "--yes"])?
            .status
            .success()
    );
    std::fs::remove_file(directory.join("untracked"))?;
    f.ok(&["worktree", "remove", worktree, "--yes"])?;
    assert!(!directory.exists());
    Ok(())
}
