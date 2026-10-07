use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use muxy_terminal::pty::{Pty, PtyEvent, PtySize, SpawnRequest};

type TestResult = Result<(), Box<dyn Error>>;

const TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn cat_echoes_written_input_and_closes_after_kill() -> TestResult {
    let mut pty = Pty::spawn(request("/bin/cat", &[]))?;
    let (sender, receiver) = channel();
    let reader = pty.start_reader(sender)?;

    pty.write(b"abc\n")?;
    let echoed = wait_for(&receiver, b"abc")?;
    assert!(text(&echoed).contains("abc"), "{echoed:?}");

    pty.kill()?;
    collect_until_closed(&receiver)?;
    let status = pty.wait()?;
    reader.join().map_err(|_| "reader thread panicked")?;

    assert_eq!(status.signal, Some(1));
    Ok(())
}

#[test]
fn resize_is_visible_to_stty() -> TestResult {
    let mut pty = Pty::spawn(shell("read line; stty size"))?;
    let (sender, receiver) = channel();
    let reader = pty.start_reader(sender)?;

    pty.resize(PtySize {
        cols: 120,
        rows: 40,
    })?;
    pty.write(b"go\n")?;
    let output = collect_until_closed(&receiver)?;
    let status = pty.wait()?;
    reader.join().map_err(|_| "reader thread panicked")?;

    assert!(text(&output).contains("40 120"), "{output:?}");
    assert_eq!(status.code, Some(0));
    Ok(())
}

fn shell(script: &str) -> SpawnRequest {
    request("/bin/sh", &["-c", script])
}

fn request(program: &str, args: &[&str]) -> SpawnRequest {
    SpawnRequest {
        clear_env: false,
        program: PathBuf::from(program),
        args: args.iter().map(OsString::from).collect(),
        cwd: env::temp_dir(),
        env: Vec::new(),
        size: PtySize { cols: 80, rows: 24 },
    }
}

fn collect_until_closed(receiver: &Receiver<PtyEvent>) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut output = Vec::new();
    loop {
        match receiver.recv_timeout(TIMEOUT)? {
            PtyEvent::Output(bytes) => output.extend(bytes),
            PtyEvent::Closed => return Ok(output),
        }
    }
}

fn wait_for(receiver: &Receiver<PtyEvent>, needle: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let deadline = Instant::now() + TIMEOUT;
    let mut output = Vec::new();
    while !contains(&output, needle) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining)? {
            PtyEvent::Output(bytes) => output.extend(bytes),
            PtyEvent::Closed => return Err("pty closed before expected output".into()),
        }
    }
    Ok(output)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
