use std::error::Error;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use muxy_app_core::browser::api::{self, CallError, Request};
use serde_json::{Map, Value};

const REPLY_TIME: Duration = Duration::from_secs(10);
const APP_START_TIME: Duration = Duration::from_secs(20);

pub(crate) fn call_app(
    verb: &str,
    args: Map<String, Value>,
    starts_app: bool,
    extra_time: Duration,
) -> Result<Value, Box<dyn Error>> {
    let socket = api::socket_path(&muxy_core::dirs::muxy_dir()?);
    let timeout = reply_time()? + extra_time;
    let request = Request::new(verb, args);
    let reply = match api::call(&socket, &request, timeout) {
        Err(CallError::NotRunning) if starts_app && start_app() => {
            call_once_listening(&socket, &request, timeout)
        }
        reply => reply,
    }?;
    Ok(reply)
}

fn start_app() -> bool {
    !crate::terminal::over_ssh() && crate::open::desktop_app(None)
}

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
