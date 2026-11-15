use std::error::Error;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use muxy_protocol::transport::LocalListener;
use serde_json::{Value, json};

fn command(directory: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_muxy"));
    command
        .args(arguments)
        .env("MUXY_DIR", directory)
        .env("MUXY_CLI_TIMEOUT", "5")
        .env("SSH_CONNECTION", "test")
        .env("NO_COLOR", "1");
    command
}

fn exchange(arguments: &[&str], reply: &[u8]) -> Result<(Output, Value), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let listener = LocalListener::bind(directory.path().join("app.sock"))?;
    listener.set_nonblocking(true)?;
    let reply = reply.to_vec();
    let server = thread::spawn(move || -> io::Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        };
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let mut bytes = Vec::new();
        (&mut stream).take(1024 * 1024).read_to_end(&mut bytes)?;
        let request = serde_json::from_slice(&bytes)?;
        stream.write_all(&reply)?;
        Ok(request)
    });
    let output = command(directory.path(), arguments).output()?;
    let request = server.join().map_err(|_| "app fixture panicked")??;
    assert!(!directory.path().join("server.sock").exists());
    Ok((output, request))
}

#[test]
fn commands_share_the_app_socket_and_preserve_requests_and_replies() -> Result<(), Box<dyn Error>> {
    for (arguments, request, value, expected) in [
        (
            vec!["browser", "list", "--project", "Demo", "--json"],
            json!({"verb": "browser.list", "args": {"project": "Demo"}}),
            json!([{"tabId": "tab-1", "url": "https://example.com"}]),
            json!([{"tabId": "tab-1", "url": "https://example.com"}]),
        ),
        (
            vec!["extension", "status", "sample", "--json"],
            json!({"verb": "extension.status", "args": {"name": "sample"}}),
            json!({"name": "sample", "enabled": true}),
            json!({"name": "sample", "enabled": true}),
        ),
        (
            vec!["config", "reload", "--json"],
            json!({"verb": "settings.reload", "args": {}}),
            Value::Null,
            json!({"applied": true}),
        ),
    ] {
        let reply = json!({"ok": true, "value": value}).to_string();
        let (output, received) = exchange(&arguments, reply.as_bytes())?;
        assert_eq!(received, request);
        assert!(output.status.success(), "{:?}", output.stderr);
        assert!(output.stderr.is_empty());
        assert_eq!(serde_json::from_slice::<Value>(&output.stdout)?, expected);
    }
    Ok(())
}

#[test]
fn app_failures_and_invalid_replies_remain_command_errors() -> Result<(), Box<dyn Error>> {
    for (reply, message) in [
        (
            br#"{"ok":false,"error":"project is unavailable"}"#.as_slice(),
            "project is unavailable",
        ),
        (b"not json", "invalid reply from Muxy"),
        (b"", "no response from Muxy"),
    ] {
        let (output, _) = exchange(&["browser", "list", "--json"], reply)?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr)?,
            format!("muxy: {message}\n")
        );
    }
    Ok(())
}

#[test]
fn timeout_validation_precedes_connecting_and_missing_apps_keep_command_policy()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    for timeout in ["0", "-1", "1.5", "invalid"] {
        let output = command(directory.path(), &["browser", "list"])
            .env("MUXY_CLI_TIMEOUT", timeout)
            .output()?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr)?,
            "muxy: MUXY_CLI_TIMEOUT must be a whole number of seconds\n"
        );
    }
    for arguments in [vec!["browser", "list"], vec!["extension", "list"]] {
        let output = command(directory.path(), &arguments).output()?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr)?,
            "muxy: the Muxy desktop app is not running\n"
        );
    }
    let output = command(directory.path(), &["config", "reload", "--json"]).output()?;
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout)?,
        json!({"applied": false})
    );
    assert!(!directory.path().join("app.sock").exists());
    assert!(!directory.path().join("server.sock").exists());
    Ok(())
}
