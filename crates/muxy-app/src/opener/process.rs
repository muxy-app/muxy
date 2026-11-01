use std::io;
use std::process::{Command, Stdio};
use std::time::Duration;

use muxy_ui::tr;

/// Starts an opener and waits for it to hand the file over. The program it
/// opens may keep running, so only the opener itself is ever killed.
pub(super) fn launch(command: &mut Command, timeout: Duration) -> io::Result<()> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let failure = match muxy_core::process::wait_timeout(&mut child, timeout) {
        Ok(Some(status)) if status.success() => return Ok(()),
        Ok(Some(status)) => {
            return Err(io::Error::other(
                tr!("Opener exited with %@", status.to_string()).to_string(),
            ));
        }
        Ok(None) => io::Error::new(io::ErrorKind::TimedOut, tr!("Opener timed out").to_string()),
        Err(error) => error,
    };
    let _ = child.kill();
    let _ = child.wait();
    Err(failure)
}
