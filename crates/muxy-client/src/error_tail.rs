use std::collections::VecDeque;
use std::io::{self, Read};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use crate::bridge::BridgeExit;

/// How much of a bridge's error output is kept for messages.
const KEPT: usize = 8 * 1024;

/// The end of a bridge's error output and how it exited, to explain a
/// connection that failed before the bridge was ready.
#[derive(Debug, Default)]
pub struct ErrorTail {
    state: Mutex<TailState>,
    ended: Condvar,
}

#[derive(Debug, Default)]
struct TailState {
    bytes: VecDeque<u8>,
    status: Option<i32>,
    ended: bool,
}

impl ErrorTail {
    /// Drains `stderr` on a thread for the child's whole life, so the child
    /// never blocks on a full pipe.
    pub(crate) fn drain(stderr: Option<impl Read + Send + 'static>) -> io::Result<Arc<Self>> {
        let tail = Arc::new(Self::default());
        let filling = Arc::clone(&tail);
        match stderr {
            Some(stderr) => {
                thread::Builder::new()
                    .name("muxy-ssh-stderr".into())
                    .spawn(move || filling.fill(stderr))?;
            }
            None => filling.finish(None),
        }
        Ok(tail)
    }

    fn fill(&self, mut stderr: impl Read) {
        let mut chunk = [0; 4096];
        loop {
            match stderr.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => self.push(&chunk[..read]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        self.finish(None);
    }

    /// Adds error output, keeping only its end.
    pub fn push(&self, bytes: &[u8]) {
        let mut state = self.lock();
        state.bytes.extend(bytes);
        let excess = state.bytes.len().saturating_sub(KEPT);
        state.bytes.drain(..excess);
    }

    /// The output ended, with the exit status if one is known. Only the
    /// first call counts.
    pub fn finish(&self, status: Option<i32>) {
        let mut state = self.lock();
        if !state.ended {
            state.status = status;
            state.ended = true;
        }
        drop(state);
        self.ended.notify_all();
    }

    /// Waits up to `timeout` for the output to end, then tells how it ended.
    pub fn wait(&self, timeout: Duration) -> BridgeExit {
        let (mut state, _) = self
            .ended
            .wait_timeout_while(self.lock(), timeout, |state| !state.ended)
            .unwrap_or_else(PoisonError::into_inner);
        BridgeExit {
            status: state.status,
            stderr: String::from_utf8_lossy(state.bytes.make_contiguous()).into_owned(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, TailState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_end_of_the_output_and_the_first_exit_status() {
        let tail = ErrorTail::default();
        tail.push(&vec![b'a'; KEPT]);
        tail.push(b"last");
        tail.finish(Some(255));
        tail.finish(Some(0));
        let exit = tail.wait(Duration::ZERO);
        assert_eq!(exit.status, Some(255));
        assert_eq!(exit.stderr.len(), KEPT);
        assert!(exit.stderr.ends_with("last"));
    }

    #[test]
    fn a_drained_pipe_ends_when_its_writer_closes() -> io::Result<()> {
        let tail = ErrorTail::drain(Some(io::Cursor::new(b"refused".to_vec())))?;
        let exit = tail.wait(Duration::from_secs(5));
        assert_eq!((exit.status, exit.stderr.as_str()), (None, "refused"));
        Ok(())
    }

    #[test]
    fn waiting_gives_up_on_output_that_has_not_ended() {
        let tail = ErrorTail::default();
        tail.push(b"partial");
        assert_eq!(tail.wait(Duration::ZERO).stderr, "partial");
    }
}
