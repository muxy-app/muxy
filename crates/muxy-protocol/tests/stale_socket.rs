mod support;

use std::error::Error;
use std::io::{self, Read};
use std::time::{Duration, Instant};
use std::{env, thread};

use muxy_protocol::transport::{Listener, UnixSocketListener, connect};
use support::{TempFolder, run_test_in_child};

type TestResult = Result<(), Box<dyn Error>>;

const TIMEOUT: Duration = Duration::from_secs(5);

const LISTEN: &str = "MUXY_TEST_LISTEN";

#[test]
fn listens_for_the_parent() -> TestResult {
    let Some(path) = env::var_os(LISTEN) else {
        return Ok(());
    };
    let _listener = UnixSocketListener::bind(path)?;
    io::stdin().read_to_end(&mut Vec::new())?;
    Ok(())
}

#[test]
fn bind_replaces_a_socket_left_by_a_killed_process() -> TestResult {
    let path = TempFolder::new()?;
    let socket = path.socket();
    let mut child = run_test_in_child(
        "listens_for_the_parent",
        LISTEN,
        socket.to_str().ok_or("path is not UTF-8")?,
    )?;
    let deadline = Instant::now() + TIMEOUT;
    while connect(&socket).is_err() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err("the child never listened".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.kill()?;
    child.wait()?;
    assert!(socket.exists());
    assert_eq!(
        connect(&socket).err().map(|error| error.kind()),
        Some(io::ErrorKind::ConnectionRefused)
    );

    let listener = UnixSocketListener::bind(&socket)?;
    let _client = connect(&socket)?;
    let _server = listener.accept()?;
    Ok(())
}
