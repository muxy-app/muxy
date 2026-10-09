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
        Ok(self.command(args).output()?)
    }
    fn command(&self, args: &[impl AsRef<std::ffi::OsStr>]) -> Command {
        let mut command = Command::new(support::binary());
        command
            .args(args)
            .env("MUXY_DIR", &self.profile)
            .env("XDG_CONFIG_HOME", self.profile.join("config"))
            .env_remove("MUXY_SERVER_BIN")
            .env_remove("MUXY_PANE_ID")
            .env_remove("MUXY_SESSION_ID")
            .env_remove("MUXY_SERVER_ID")
            .current_dir(self.directory.path());
        command
    }
    /// Runs `args` as if from inside the terminal `session`.
    fn inside(&self, session: &str, args: &[&str]) -> Result<Output> {
        let server = self.client()?.catalog()?.server.to_string();
        Ok(self
            .command(args)
            .env("MUXY_SESSION_ID", session)
            .env("MUXY_SERVER_ID", server)
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
    assert!(f.json(&["activity", "list", "--json"])?.is_object());
    assert_eq!(f.ok(&["activity", "list"])?, "");
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
        "worktree",
        "create",
        &project,
        "feature",
        "--directory",
        path,
        "--json",
    ])?;
    let worktree = created["id"].as_str().ok_or("worktree")?;
    assert_eq!(created["name"], "feature");
    assert!(directory.is_dir());
    assert!(
        f.json(&["worktree", "list", &project, "--json"])?
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

#[test]
fn started_commands_can_be_awaited_and_session_ids_default_to_the_calling_terminal() -> Result {
    let f = Fixture::new()?;
    let project = f.project()?;
    f.shell()?;
    let session = f
        .ok(&[
            "session",
            "create",
            &project,
            "--",
            "printf",
            "'%s_%s\\n'",
            "WAIT",
            "READY",
        ])?
        .trim()
        .to_owned();
    let ready = f.ok(&[
        "session",
        "wait",
        &session,
        "--text",
        "wait_ready",
        "--ignore-case",
    ])?;
    assert!(ready.contains("WAIT_READY"), "{ready}");
    let missing = f.run(&[
        "session",
        "wait",
        &session,
        "--text",
        "NEVER",
        "--timeout-ms",
        "200",
    ])?;
    assert!(!missing.status.success());
    assert!(String::from_utf8(missing.stderr)?.contains("timed out"));
    let unknown = f.run(&["session", "read-screen"])?;
    assert!(!unknown.status.success());
    assert!(String::from_utf8(unknown.stderr)?.contains("give a session ID"));
    let screen = f.inside(&session, &["session", "read-screen"])?;
    assert!(String::from_utf8(screen.stdout)?.contains("WAIT_READY"));
    let sibling = f.inside(&session, &["session", "create", "--json"])?;
    assert!(sibling.status.success(), "{sibling:?}");
    let sibling: Value = serde_json::from_slice(&sibling.stdout)?;
    assert_eq!(sibling["project_id"], project);
    assert!(
        f.inside(&session, &["session", "send", "exit"])?
            .status
            .success()
    );
    assert!(
        f.inside(&session, &["session", "send-keys", "Enter"])?
            .status
            .success()
    );
    assert_eq!(
        f.ok(&["session", "wait", &session, "--exit"])?,
        "exited 0\n"
    );
    assert_eq!(
        f.json(&["session", "wait", &session, "--exit", "--json"])?["ended"],
        json!({"Exited":0})
    );
    let ready = f.ok(&["session", "wait", &session, "--text", "WAIT_READY"])?;
    assert!(ready.contains("WAIT_READY"), "{ready}");
    let ended = f.run(&["session", "wait", &session, "--text", "NEVER"])?;
    assert!(String::from_utf8(ended.stderr)?.contains("ended before the text appeared"));
    let sibling = sibling["id"].as_str().ok_or("sibling")?;
    f.ok(&["session", "end", sibling, "--yes"])?;
    Ok(())
}

#[test]
fn initial_exec_command_can_be_awaited_to_completion() -> Result {
    let f = Fixture::new()?;
    let project = f.project()?;
    f.shell()?;
    let session = f
        .ok(&["session", "create", &project, "--", "exec sh -c 'exit 7'"])?
        .trim()
        .to_owned();
    assert_eq!(
        f.json(&["session", "wait", &session, "--exit", "--json"])?["ended"],
        json!({"Exited": 7})
    );
    Ok(())
}

#[test]
fn worktrees_use_the_default_location_and_run_hooks_only_when_asked() -> Result {
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
    let trees = f.directory.path().canonicalize()?.join("trees");
    std::fs::write(
        f.profile.join("settings.toml"),
        format!(
            "[worktrees.default_location]\nparent_path = \"{}\"\n",
            trees.display()
        ),
    )?;
    std::fs::create_dir(f.directory.path().join(".muxy"))?;
    std::fs::write(
        f.directory.path().join(".muxy/worktree.json"),
        r#"{"setup": ["touch \"$MUXY_PROJECT_PATH/setup-ran\""],
            "teardown": ["touch \"$MUXY_PROJECT_PATH/teardown-ran\""]}"#,
    )?;
    let project = f.project()?;
    let plain = f.json(&["worktree", "create", &project, "plain", "--json"])?;
    let plain_directory = trees.join("Test/plain");
    assert_eq!(plain["directory"], plain_directory.to_str().ok_or("path")?);
    assert!(plain_directory.is_dir());
    assert!(!f.directory.path().join("setup-ran").exists());
    let hooked = f.run(&["worktree", "create", &project, "hooked", "--hooks"])?;
    assert!(hooked.status.success(), "{hooked:?}");
    assert!(String::from_utf8(hooked.stderr)?.contains("setup: touch"));
    assert!(f.directory.path().join("setup-ran").exists());
    let listed = f.ok(&["worktree", "list", &project])?;
    assert_eq!(listed.lines().count(), 3, "{listed}");
    assert!(listed.lines().any(|line| {
        let columns: Vec<_> = line.split('\t').collect();
        columns[1] == "plain" && columns[2] == plain_directory.to_str().unwrap_or_default()
    }));
    std::fs::write(plain_directory.join("untracked"), "keep")?;
    let refused = f.run(&["worktree", "remove", "plain", "--yes"])?;
    assert!(!refused.status.success());
    assert!(String::from_utf8(refused.stderr)?.contains("--force"));
    f.ok(&["worktree", "remove", "plain", "--yes", "--force"])?;
    assert!(!plain_directory.exists());
    assert!(!f.directory.path().join("teardown-ran").exists());
    f.ok(&["worktree", "remove", "hooked", "--yes", "--hooks"])?;
    assert!(f.directory.path().join("teardown-ran").exists());
    Ok(())
}

#[test]
fn projects_can_be_created_and_reused() -> Result {
    let f = Fixture::new()?;
    let created = f.json(&["project", "add", "new/app", "--create", "--json"])?;
    assert!(f.directory.path().join("new/app").is_dir());
    let reused = f.json(&["project", "add", "new/app", "--reuse", "--json"])?;
    assert_eq!(reused["id"], created["id"]);
    let duplicate = f.json(&["project", "add", "new/app", "--json"])?;
    assert_ne!(duplicate["id"], created["id"]);
    Ok(())
}
