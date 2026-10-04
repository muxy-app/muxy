use std::error::Error;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use muxy_protocol::transport::{ByteStream, ChildStream, relay};

type TestResult = Result<(), Box<dyn Error>>;

const TIMEOUT: Duration = Duration::from_secs(5);
const BLOCKED_INTERVAL: Duration = Duration::from_millis(50);
const SIGKILL: i32 = 9;

fn shell(script: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", script]);
    command
}

#[test]
fn bytes_round_trip_through_cat_and_its_exit_is_reported() -> TestResult {
    let stream = ChildStream::spawn(Command::new("cat"))?;
    let process = stream.process();
    let (mut reader, mut writer) = Box::new(stream).split()?;

    writer.write_all(b"hello\0\xff")?;
    let mut echoed = [0; 7];
    reader.read_exact(&mut echoed)?;
    assert_eq!(&echoed, b"hello\0\xff");

    drop(writer);
    let mut rest = Vec::new();
    reader.read_to_end(&mut rest)?;
    assert!(rest.is_empty());
    let status = process.wait_timeout(TIMEOUT)?.ok_or("cat did not exit")?;
    assert!(status.success(), "{status:?}");
    Ok(())
}

#[test]
fn the_owner_keeps_stderr_and_reads_the_exit_status_after_stdout_ends() -> TestResult {
    let mut command = shell("printf out; printf oops >&2; exit 3");
    command.stderr(Stdio::piped());
    let mut stream = ChildStream::spawn(command)?;
    let mut stderr = stream.take_stderr().ok_or("stderr was not piped")?;
    assert!(stream.take_stderr().is_none());
    let process = stream.process();
    let (mut reader, _writer) = Box::new(stream).split()?;

    let mut output = Vec::new();
    reader.read_to_end(&mut output)?;
    assert_eq!(output, b"out");
    let mut errors = String::new();
    stderr.read_to_string(&mut errors)?;
    assert_eq!(errors, "oops");
    assert_eq!(
        process
            .wait_timeout(TIMEOUT)?
            .and_then(|status| status.code()),
        Some(3)
    );
    Ok(())
}

#[test]
fn cancelling_wakes_a_blocked_read_and_reaps_the_child() -> TestResult {
    let stream = ChildStream::spawn(shell("exec sleep 60"))?;
    let process = stream.process();
    let cancellation = stream.cancellation()?;
    let (mut reader, _writer) = Box::new(stream).split()?;
    let (ready, started) = mpsc::channel();
    let (sender, finished) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _ = ready.send(());
        let _ = sender.send(reader.read(&mut [0]).ok());
    });

    started.recv_timeout(TIMEOUT)?;
    assert!(matches!(
        finished.recv_timeout(BLOCKED_INTERVAL),
        Err(RecvTimeoutError::Timeout)
    ));
    assert!(process.wait_timeout(Duration::ZERO)?.is_none());
    thread::spawn(move || {
        cancellation.cancel();
        cancellation.cancel();
    })
    .join()
    .map_err(|_| "cancel panicked")?;

    assert_eq!(finished.recv_timeout(TIMEOUT)?, Some(0));
    worker.join().map_err(|_| "reader panicked")?;
    let status = process
        .wait_timeout(Duration::ZERO)?
        .ok_or("the child was not reaped")?;
    assert_eq!(status.signal(), Some(SIGKILL));
    Ok(())
}

#[test]
fn cancelling_wakes_a_read_even_while_a_grandchild_holds_the_childs_end() -> TestResult {
    let stream = ChildStream::spawn(shell("sleep 30 & echo $!; exec sleep 60"))?;
    let cancellation = stream.cancellation()?;
    let (reader, _writer) = Box::new(stream).split()?;
    let mut reader = BufReader::new(reader);
    let mut grandchild = String::new();
    reader.read_line(&mut grandchild)?;
    let (sender, finished) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _ = sender.send(reader.read(&mut [0]).ok());
    });

    cancellation.cancel();
    let woke = finished.recv_timeout(TIMEOUT);
    let _ = Command::new("kill").arg(grandchild.trim()).status();
    assert_eq!(woke?, Some(0));
    worker.join().map_err(|_| "reader panicked")?;
    Ok(())
}

#[test]
fn dropping_an_unsplit_stream_kills_and_reaps_the_child() -> TestResult {
    let stream = ChildStream::spawn(shell("exec sleep 60"))?;
    let process = stream.process();
    drop(stream);
    let status = process
        .wait_timeout(Duration::ZERO)?
        .ok_or("the child was not reaped")?;
    assert_eq!(status.signal(), Some(SIGKILL));
    Ok(())
}

#[test]
fn relay_flushes_every_chunk_and_half_closes_when_input_ends() -> TestResult {
    let (stream, mut server) = UnixStream::pair()?;
    let (input, mut stdin) = UnixStream::pair()?;
    let (output, mut stdout) = UnixStream::pair()?;
    server.set_read_timeout(Some(TIMEOUT))?;
    stdout.set_read_timeout(Some(TIMEOUT))?;
    // A buffered output holds anything the relay forgets to flush.
    let relaying = thread::spawn(move || relay(Box::new(stream), input, BufWriter::new(output)));

    stdin.write_all(b"request")?;
    let mut request = [0; 7];
    server.read_exact(&mut request)?;
    assert_eq!(&request, b"request");
    server.write_all(b"reply without a newline")?;
    let mut reply = [0; 23];
    stdout.read_exact(&mut reply)?;
    assert_eq!(&reply, b"reply without a newline");

    stdin.shutdown(Shutdown::Write)?;
    let mut rest = Vec::new();
    server.read_to_end(&mut rest)?;
    assert!(rest.is_empty());
    server.write_all(b"after input ended")?;
    let mut late = [0; 17];
    stdout.read_exact(&mut late)?;
    assert_eq!(&late, b"after input ended");

    drop(server);
    relaying.join().map_err(|_| "relay panicked")??;
    stdout.read_to_end(&mut rest)?;
    assert!(rest.is_empty());
    Ok(())
}

#[test]
fn relay_ends_normally_and_closes_the_stream_when_output_is_gone() -> TestResult {
    let (stream, mut server) = UnixStream::pair()?;
    let (input, _stdin) = UnixStream::pair()?;
    let (output, stdout) = UnixStream::pair()?;
    server.set_read_timeout(Some(TIMEOUT))?;
    drop(stdout);
    let relaying = thread::spawn(move || relay(Box::new(stream), input, output));

    server.write_all(b"nobody reads this")?;
    relaying.join().map_err(|_| "relay panicked")??;
    assert_eq!(server.read(&mut [0; 8])?, 0);
    Ok(())
}
