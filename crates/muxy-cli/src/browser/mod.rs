//! `muxy browser`: drives the built-in browser tabs of the desktop app on
//! this computer. Each command is one request to the app over its socket.

mod args;

use std::error::Error;
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use muxy_app_core::browser::api::{self, CallError, Request};
use serde_json::Value;

use args::Output;
pub(crate) use args::{Invocation, parse};

/// How long the app has to answer, unless `MUXY_CLI_TIMEOUT` says otherwise.
const REPLY_TIME: Duration = Duration::from_secs(10);
/// How long a desktop app that `open` started has to begin listening.
const APP_START_TIME: Duration = Duration::from_secs(20);
const INVALID_REPLY: &str = "invalid reply from Muxy";

pub(crate) fn run(invocation: Invocation) -> Result<(), Box<dyn Error>> {
    let socket = api::socket_path(&muxy_core::dirs::muxy_dir()?);
    let timeout = reply_time()? + invocation.extra_time;
    let starts_app = invocation.verb == "browser.open";
    let request = Request::new(invocation.verb, invocation.args);
    let reply = match api::call(&socket, &request, timeout) {
        Err(CallError::NotRunning) if starts_app && start_app() => {
            call_once_listening(&socket, &request, timeout)
        }
        reply => reply,
    }?;
    print(invocation.output, reply)
}

/// Starts the desktop app as `muxy <folder>` does, but never over SSH,
/// where it would open away from the user.
fn start_app() -> bool {
    !crate::terminal::over_ssh() && crate::open::desktop_app(None)
}

/// Sends `request` once the app that was just started listens.
fn call_once_listening(
    socket: &Path,
    request: &Request,
    timeout: Duration,
) -> Result<Value, CallError> {
    let deadline = Instant::now() + APP_START_TIME;
    loop {
        match api::call(socket, request, timeout) {
            Err(CallError::NotRunning) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(100));
            }
            reply => return reply,
        }
    }
}

fn reply_time() -> Result<Duration, Box<dyn Error>> {
    let Some(seconds) = std::env::var_os("MUXY_CLI_TIMEOUT").filter(|value| !value.is_empty())
    else {
        return Ok(REPLY_TIME);
    };
    seconds
        .to_str()
        .and_then(|seconds| seconds.parse().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .ok_or_else(|| "MUXY_CLI_TIMEOUT must be a whole number of seconds".into())
}

fn print(output: Output, reply: Value) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout().lock();
    match output {
        Output::Ok => writeln!(stdout, "ok")?,
        Output::Json => writeln!(stdout, "{reply}")?,
        Output::Text => match reply {
            Value::String(text) => writeln!(stdout, "{text}")?,
            other => writeln!(stdout, "{other}")?,
        },
        Output::Tabs => {
            for tab in reply.as_array().ok_or(INVALID_REPLY)? {
                let row: Vec<_> = ["tabId", "title", "url", "profile", "active"]
                    .iter()
                    .map(|key| cell(&tab[key]))
                    .collect();
                writeln!(stdout, "{}", row.join("\t"))?;
            }
        }
        Output::Page => writeln!(
            stdout,
            "{}\n{}\n{}",
            text(&reply["title"]),
            text(&reply["url"]),
            text(&reply["text"])
        )?,
        Output::Screenshot => writeln!(stdout, "{}", reply["png"].as_str().ok_or(INVALID_REPLY)?)?,
    }
    Ok(())
}

/// A list field kept on its line: tabs and line breaks become spaces.
fn cell(value: &Value) -> String {
    match value {
        Value::String(text) => text.replace(['\t', '\n', '\r'], " "),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}
