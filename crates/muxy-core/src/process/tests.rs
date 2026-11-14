#![allow(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::*;

fn sh(script: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", script]);
    command
}

fn limits() -> Limits<'static> {
    Limits::new(Duration::from_secs(10), 1024 * 1024)
}

fn printed_pid(output: &Output) -> Pid {
    let text = String::from_utf8(output.stdout.bytes.clone()).unwrap();
    Pid::from_raw(text.trim().parse().unwrap()).unwrap()
}

/// An orphan stays a zombie until the system reaps it, so give that a moment.
fn gone(pid: Pid) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if rustix::process::test_kill_process(pid).is_err() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn collects_output_and_the_exit_status() {
    let output = run(&mut sh("echo out; echo err >&2; exit 3"), limits()).unwrap();
    assert_eq!(output.stdout.bytes, b"out\n");
    assert_eq!(output.stderr.bytes, b"err\n");
    assert_eq!(output.status.code(), Some(3));
    assert!(!output.timed_out && !output.cancelled);
    assert!(!output.stdout.truncated && !output.stderr.truncated);
}

#[test]
fn timeout_kills_the_whole_process_group() {
    let started = Instant::now();
    let output = run(
        &mut sh("sleep 30 & echo $!; wait"),
        Limits::new(Duration::from_millis(200), 1024),
    )
    .unwrap();
    assert!(output.timed_out);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        gone(printed_pid(&output)),
        "the helper outlived the timeout"
    );
}

#[test]
fn output_past_a_stopping_cap_stops_the_command() {
    let output = run(
        &mut sh("while :; do echo yes; done"),
        Limits {
            stdout: Cap::StopAt(10),
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(output.stdout.bytes, b"yes\nyes\nye");
    assert!(output.stdout.truncated);
    assert!(!output.timed_out);
    assert!(!output.status.success());
}

#[test]
fn output_past_a_truncating_cap_is_discarded_while_the_command_finishes() {
    let output = run(
        &mut sh("head -c 100000 /dev/zero; echo done >&2"),
        Limits {
            stdout: Cap::TruncateAt(10),
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(output.stdout.bytes, [0; 10]);
    assert!(output.stdout.truncated);
    assert_eq!(output.stderr.bytes, b"done\n");
    assert!(output.status.success());
}

#[test]
fn stdin_is_written_while_output_is_read() {
    let input: Vec<u8> = (0..=250).cycle().take(4 * 1024 * 1024).collect();
    let output = run(
        &mut Command::new("cat"),
        Limits {
            stdin: Some(&input),
            ..Limits::new(Duration::from_secs(10), 8 * 1024 * 1024)
        },
    )
    .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.bytes == input);
}

#[test]
fn leftovers_holding_the_output_are_waited_for_unless_killed() {
    let output = run(&mut sh("(sleep 0.3; echo late) &"), limits()).unwrap();
    assert_eq!(output.stdout.bytes, b"late\n");

    let started = Instant::now();
    let output = run(
        &mut sh("sleep 30 & echo $!"),
        Limits {
            kill_leftovers: true,
            ..limits()
        },
    )
    .unwrap();
    assert!(output.status.success());
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        gone(printed_pid(&output)),
        "the helper outlived its command"
    );
}

#[test]
fn cancelling_stops_the_command() {
    let cancelled = AtomicBool::new(false);
    let check = || cancelled.load(Ordering::Relaxed);
    let output = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            cancelled.store(true, Ordering::Relaxed);
        });
        run(
            &mut sh("sleep 30"),
            Limits {
                cancelled: Some(&check),
                ..limits()
            },
        )
        .unwrap()
    });
    assert!(output.cancelled);
    assert!(!output.timed_out);
}

#[test]
fn a_grace_period_lets_the_command_handle_term() {
    let output = run(
        &mut sh("trap 'echo bye; exit 7' TERM; sleep 30 & wait"),
        Limits {
            term_grace: Duration::from_secs(5),
            ..Limits::new(Duration::from_millis(100), 1024)
        },
    )
    .unwrap();
    assert!(output.timed_out);
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout.bytes, b"bye\n");
}

#[test]
fn wait_timeout_reaps_an_exited_child_and_leaves_a_running_one() {
    let mut running = Command::new("sleep").arg("30").spawn().unwrap();
    let started = Instant::now();
    assert!(
        wait_timeout(&mut running, Duration::from_millis(100))
            .unwrap()
            .is_none()
    );
    assert!(started.elapsed() >= Duration::from_millis(100));
    running.kill().unwrap();
    running.wait().unwrap();

    let mut finished = Command::new("true").spawn().unwrap();
    let status = wait_timeout(&mut finished, Duration::from_secs(10)).unwrap();
    assert!(status.is_some_and(|status| status.success()));
}
