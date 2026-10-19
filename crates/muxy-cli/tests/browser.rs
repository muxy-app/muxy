//! Browser panes are Unix-only for now, so these tests run only on Unix.
#![cfg(unix)]

mod support;

use std::io::{self, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Command, Output};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use muxy_app_core::browser::api::{self, Request};
use serde_json::{Map, Value, json};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// A profile folder under `/tmp`: macOS limits socket paths to about 104
/// bytes, and the app's socket lives in the profile.
fn profile() -> Result<tempfile::TempDir> {
    Ok(tempfile::Builder::new()
        .prefix("muxy-browser-")
        .tempdir_in("/tmp")?)
}

fn browser(profile: &Path, args: &[&str]) -> Result<Output> {
    Ok(Command::new(support::binary())
        .arg("browser")
        .args(args)
        .env("MUXY_DIR", profile)
        .env_remove("MUXY_CLI_TIMEOUT")
        .output()?)
}

/// Stands in for the desktop app: answers one request with `reply` and hands
/// the request back, giving up if nobody asks within ten seconds.
fn app(
    profile: &Path,
    reply: std::result::Result<Value, String>,
) -> Result<JoinHandle<io::Result<Request>>> {
    let listener = UnixListener::bind(api::socket_path(profile))?;
    listener.set_nonblocking(true)?;
    Ok(thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let stream = loop {
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
        let request = Request::read(&stream)?;
        (&stream).write_all(&api::encode_reply(&reply))?;
        Ok(request)
    }))
}

/// Runs `muxy browser args` against an app that answers with `reply`.
fn exchange(args: &[&str], reply: std::result::Result<Value, String>) -> Result<(Request, Output)> {
    let profile = profile()?;
    let app = app(profile.path(), reply)?;
    let output = browser(profile.path(), args)?;
    let request = app.join().map_err(|_| "the app thread panicked")??;
    Ok((request, output))
}

fn request(verb: &str, args: Value) -> Result<Request> {
    let args: Map<String, Value> = serde_json::from_value(args)?;
    Ok(Request::new(verb, args))
}

fn stdout(output: &Output) -> Result<&str> {
    assert!(output.status.success(), "{output:?}");
    Ok(std::str::from_utf8(&output.stdout)?)
}

#[test]
fn open_prints_the_new_tab_id() -> Result {
    let (sent, output) = exchange(
        &["open", "example.com", "--split", "--project", "App"],
        Ok(json!("tab-1")),
    )?;
    assert_eq!(
        sent,
        request(
            "browser.open",
            json!({"url": "example.com", "split": true, "project": "App"})
        )?
    );
    assert_eq!(stdout(&output)?, "tab-1\n");
    Ok(())
}

#[test]
fn list_prints_a_line_per_tab_or_json() -> Result {
    let tabs = json!([
        {"tabId": "t1", "title": "Docs\tHome\nPage", "url": "https://a.test/",
         "profile": "Default", "active": true},
        {"tabId": "t2", "title": "Blank", "url": "about:blank",
         "profile": null, "active": false},
    ]);
    let (sent, output) = exchange(&["list", "--project", "App"], Ok(tabs.clone()))?;
    assert_eq!(sent, request("browser.list", json!({"project": "App"}))?);
    assert_eq!(
        stdout(&output)?,
        "t1\tDocs Home Page\thttps://a.test/\tDefault\ttrue\n\
         t2\tBlank\tabout:blank\t\tfalse\n"
    );
    let (_, output) = exchange(&["list", "--json"], Ok(tabs.clone()))?;
    assert_eq!(serde_json::from_str::<Value>(stdout(&output)?)?, tabs);
    Ok(())
}

#[test]
fn read_prints_title_url_and_text() -> Result {
    let page = json!({"title": "Example", "url": "https://example.com/", "text": "Hello\nworld"});
    let (sent, output) = exchange(&["read", "tab-1"], Ok(page))?;
    assert_eq!(sent, request("browser.read", json!({"tabId": "tab-1"}))?);
    assert_eq!(
        stdout(&output)?,
        "Example\nhttps://example.com/\nHello\nworld\n"
    );
    Ok(())
}

#[test]
fn eval_prints_the_result_as_json() -> Result {
    let (sent, output) = exchange(
        &["eval", "tab-1", "document.title"],
        Ok(json!("Example Domain")),
    )?;
    assert_eq!(
        sent,
        request(
            "browser.eval",
            json!({"tabId": "tab-1", "script": "document.title"})
        )?
    );
    assert_eq!(stdout(&output)?, "\"Example Domain\"\n");
    let (_, output) = exchange(&["eval", "tab-1", "6 * 7"], Ok(json!(42)))?;
    assert_eq!(stdout(&output)?, "42\n");
    Ok(())
}

#[test]
fn screenshot_prints_raw_base64() -> Result {
    let (sent, output) = exchange(&["screenshot", "tab-1"], Ok(json!({"png": "iVBORw0KGgo="})))?;
    assert_eq!(
        sent,
        request("browser.screenshot", json!({"tabId": "tab-1"}))?
    );
    assert_eq!(stdout(&output)?, "iVBORw0KGgo=\n");
    Ok(())
}

#[test]
fn app_errors_fail_the_command() -> Result {
    let (_, output) = exchange(
        &["click", "missing", "#go"],
        Err("browser tab not found missing".into()),
    )?;
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        std::str::from_utf8(&output.stderr)?,
        "muxy: browser tab not found missing\n"
    );
    Ok(())
}

#[test]
fn commands_other_than_open_need_the_app_running() -> Result {
    let profile = profile()?;
    let output = browser(profile.path(), &["list"])?;
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        std::str::from_utf8(&output.stderr)?,
        "muxy: the Muxy desktop app is not running\n"
    );
    Ok(())
}
