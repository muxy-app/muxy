use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use muxy_terminal::pty::{ExitStatus, Pty, PtyEvent, PtySize, ReaderHandle, SpawnRequest};

type TestResult = Result<(), Box<dyn Error>>;

const TIMEOUT: Duration = Duration::from_secs(10);
const CURSOR_QUERY: &[u8] = b"\x1b[6n";

#[cfg(unix)]
const ENTER: &[u8] = b"\n";
#[cfg(windows)]
const ENTER: &[u8] = b"\r";

/// `SIGHUP` on Unix; `TerminateProcess` exit code 1 on Windows.
#[cfg(unix)]
const KILLED: ExitStatus = ExitStatus {
    code: None,
    signal: Some(1),
};
#[cfg(windows)]
const KILLED: ExitStatus = ExitStatus {
    code: Some(1),
    signal: None,
};

#[test]
fn interactive_program_echoes_input_and_closes_after_kill() -> TestResult {
    #[cfg(unix)]
    let mut session = Session::start(request("/bin/cat", &[]))?;
    #[cfg(windows)]
    let mut session = Session::start(request("cmd.exe", &["/d"]))?;

    session.type_line(b"abc")?;
    session.wait_for(b"abc")?;

    session.pty.kill()?;
    session.until_closed()?;

    assert_eq!(session.finish()?, KILLED);
    Ok(())
}

#[test]
fn output_and_exit_code_reach_the_owner() -> TestResult {
    #[cfg(unix)]
    let mut session = Session::start(shell("echo hello; exit 3"))?;
    #[cfg(windows)]
    let mut session = Session::start(shell("echo hello& exit 3"))?;

    let output = session.until_closed()?;

    assert!(text(&output).contains("hello"), "{output:?}");
    assert_eq!(session.finish()?.code, Some(3));
    Ok(())
}

/// 259 is `STILL_ACTIVE`, which a naive exit-code check reads as running.
#[cfg(windows)]
#[test]
fn exit_code_259_is_reported_as_an_exit() -> TestResult {
    let mut session = Session::start(shell("exit 259"))?;
    session.until_closed()?;

    assert_eq!(
        session.pty.try_wait().and_then(|status| status.code),
        Some(259)
    );
    assert_eq!(session.finish()?.code, Some(259));
    Ok(())
}

#[test]
fn shell_session_ends_after_exit() -> TestResult {
    #[cfg(unix)]
    let mut session = Session::start(request("/bin/sh", &[]))?;
    #[cfg(windows)]
    let mut session = Session::start(request("cmd.exe", &["/d"]))?;

    session.type_line(b"exit")?;
    session.until_closed()?;

    assert_eq!(session.finish()?.code, Some(0));
    Ok(())
}

#[test]
fn resize_reaches_the_running_child() -> TestResult {
    #[cfg(unix)]
    let mut session = Session::start(shell("read line; stty size"))?;
    #[cfg(windows)]
    let mut session = Session::start(shell("set /p line=& mode con"))?;

    session.pty.resize(PtySize {
        cols: 120,
        rows: 40,
    })?;
    session.type_line(b"go")?;
    let output = text(&session.until_closed()?);

    #[cfg(unix)]
    assert!(output.contains("40 120"), "{output:?}");
    #[cfg(windows)]
    assert!(
        output.contains("Columns:        120") && output.contains("Lines:          40"),
        "{output:?}"
    );
    assert_eq!(session.finish()?.code, Some(0));
    Ok(())
}

#[test]
fn resizing_after_the_child_exits_is_harmless() -> TestResult {
    let mut session = Session::start(shell("exit 0"))?;
    session.until_closed()?;

    session.pty.resize(PtySize {
        cols: 100,
        rows: 30,
    })?;

    assert_eq!(session.finish()?.code, Some(0));
    Ok(())
}

/// A PTY whose output is read on its own thread, answering `ConPTY`'s cursor
/// position query as a terminal would.
struct Session {
    pty: Pty,
    events: Receiver<PtyEvent>,
    reader: ReaderHandle,
    output: Vec<u8>,
}

impl Session {
    fn start(request: SpawnRequest) -> Result<Self, Box<dyn Error>> {
        let pty = Pty::spawn(request)?;
        let (sender, events) = channel();
        let reader = pty.start_reader(sender)?;
        Ok(Self {
            pty,
            events,
            reader,
            output: Vec::new(),
        })
    }

    fn type_line(&mut self, line: &[u8]) -> TestResult {
        self.pty.write(&[line, ENTER].concat())?;
        Ok(())
    }

    fn wait_for(&mut self, needle: &[u8]) -> TestResult {
        let deadline = Instant::now() + TIMEOUT;
        while !contains(&self.output, needle) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if !self.receive(remaining)? {
                return Err(format!("pty closed before {:?}", text(needle)).into());
            }
        }
        Ok(())
    }

    fn until_closed(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        while self.receive(TIMEOUT)? {}
        Ok(self.output.clone())
    }

    /// Whether the output is still open.
    fn receive(&mut self, timeout: Duration) -> Result<bool, Box<dyn Error>> {
        match self.events.recv_timeout(timeout)? {
            PtyEvent::Output(bytes) => {
                if contains(&bytes, CURSOR_QUERY) {
                    self.pty.write(b"\x1b[1;1R")?;
                }
                self.output.extend(bytes);
                Ok(true)
            }
            PtyEvent::Closed => Ok(false),
        }
    }

    fn finish(mut self) -> Result<ExitStatus, Box<dyn Error>> {
        let status = self.pty.wait()?;
        self.reader.join().map_err(|_| "reader thread panicked")?;
        Ok(status)
    }
}

#[cfg(unix)]
fn shell(script: &str) -> SpawnRequest {
    request("/bin/sh", &["-c", script])
}

#[cfg(windows)]
fn shell(script: &str) -> SpawnRequest {
    request("cmd.exe", &["/d", "/c", script])
}

fn request(program: &str, args: &[&str]) -> SpawnRequest {
    SpawnRequest {
        program: PathBuf::from(program),
        args: args.iter().map(OsString::from).collect(),
        cwd: env::temp_dir(),
        env: Vec::new(),
        size: PtySize { cols: 80, rows: 24 },
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
