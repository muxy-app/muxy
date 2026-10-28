use std::io::{ErrorKind, Read};
use std::ops::Deref;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};

const READ_BUFFER_SIZE: usize = 64 * 1024;
/// Output read but not yet consumed. Past this the reader stops reading, so the
/// kernel buffer fills and the program blocks, as it would in any terminal.
const MAX_UNCONSUMED_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub enum PtyEvent {
    Output(PtyOutput),
    Closed,
}

/// Bytes read from the PTY. Dropping them lets the reader read more.
#[derive(Debug)]
pub struct PtyOutput {
    bytes: Vec<u8>,
    budget: Arc<Budget>,
}

impl Deref for PtyOutput {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for PtyOutput {
    fn drop(&mut self) {
        self.budget.release(self.bytes.len());
    }
}

#[derive(Debug, Default)]
struct Budget {
    unconsumed: Mutex<usize>,
    released: Condvar,
}

impl Budget {
    fn acquire(&self, count: usize) {
        let mut unconsumed = self
            .unconsumed
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        while *unconsumed >= MAX_UNCONSUMED_BYTES {
            unconsumed = self
                .released
                .wait(unconsumed)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *unconsumed += count;
    }

    fn release(&self, count: usize) {
        let mut unconsumed = self
            .unconsumed
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *unconsumed -= count;
        self.released.notify_one();
    }
}

#[derive(Debug)]
pub struct ReaderHandle {
    thread: JoinHandle<()>,
}

impl ReaderHandle {
    pub fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }

    pub fn join(self) -> thread::Result<()> {
        self.thread.join()
    }
}

pub(crate) fn spawn_reader<T>(mut reader: Box<dyn Read + Send>, sink: Sender<T>) -> ReaderHandle
where
    T: From<PtyEvent> + Send + 'static,
{
    let thread = thread::spawn(move || {
        let budget = Arc::new(Budget::default());
        let mut buffer = vec![0; READ_BUFFER_SIZE];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    budget.acquire(count);
                    let output = PtyOutput {
                        bytes: buffer[..count].to_vec(),
                        budget: Arc::clone(&budget),
                    };
                    if sink.send(PtyEvent::Output(output).into()).is_err() {
                        return;
                    }
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        let _ = sink.send(PtyEvent::Closed.into());
    });
    ReaderHandle { thread }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use super::*;

    #[test]
    fn reader_waits_for_consumers_once_a_megabyte_is_unconsumed() {
        let input = vec![b'x'; MAX_UNCONSUMED_BYTES + 3 * READ_BUFFER_SIZE];
        let (sender, receiver) = channel::<PtyEvent>();
        let handle = spawn_reader(Box::new(Cursor::new(input)), sender);
        let mut held = Vec::new();
        let mut unconsumed = 0;
        while unconsumed < MAX_UNCONSUMED_BYTES {
            let Ok(PtyEvent::Output(output)) = receiver.recv_timeout(Duration::from_secs(10))
            else {
                panic!("the reader stopped before the budget filled");
            };
            unconsumed += output.len();
            held.push(output);
        }
        // Give a reader that ignored the budget time to send more.
        thread::sleep(Duration::from_millis(200));
        for event in receiver.try_iter() {
            if let PtyEvent::Output(output) = event {
                unconsumed += output.len();
                held.push(output);
            }
        }
        assert!(unconsumed < MAX_UNCONSUMED_BYTES + READ_BUFFER_SIZE);
        assert!(!handle.is_finished());

        drop(held);
        let mut rest = 0;
        while let PtyEvent::Output(output) = receiver.recv().unwrap() {
            rest += output.len();
        }
        assert_eq!(
            unconsumed + rest,
            MAX_UNCONSUMED_BYTES + 3 * READ_BUFFER_SIZE
        );
        handle.join().unwrap();
    }
}
